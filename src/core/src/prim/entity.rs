use std::{
	fmt,
	marker::PhantomData,
	sync::{
		atomic::{AtomicU64, Ordering::Relaxed},
		Arc,
	},
};

use crate::{
	debug::{error::ResultExt, type_id::NamedTypeId},
	mem::ptr::{Incomplete, PointeeCastExt},
};
use derive_where::derive_where;
use itertools::Itertools;
use thiserror::Error;

use super::lock::{CompCell, CompMut, CompRef, Session};

// === TypedKey === //

pub trait KeyProxy: 'static {
	type Target: ?Sized + 'static;
}

#[derive_where(Debug, Copy, Clone, Hash, Eq, PartialEq)]
#[repr(transparent)]
pub struct TypedKey<T: ?Sized + 'static> {
	_ty: PhantomData<fn(T) -> T>,
	raw: RawTypedKey,
}

impl<T: ?Sized + 'static> Default for TypedKey<T> {
	fn default() -> Self {
		Self::instance()
	}
}

impl<T: ?Sized + 'static> TypedKey<T> {
	// === Raw conversions === //

	pub unsafe fn from_raw_unchecked(raw: RawTypedKey) -> Self {
		Self {
			_ty: PhantomData,
			raw,
		}
	}

	pub fn raw(self) -> RawTypedKey {
		self.raw
	}

	// === Constructors === //

	pub fn instance() -> Self {
		Self {
			_ty: PhantomData,
			raw: RawTypedKey::Instance(NamedTypeId::of::<T>()),
		}
	}

	pub fn proxy<P>() -> Self
	where
		P: ?Sized + KeyProxy<Target = T>,
	{
		Self {
			_ty: PhantomData,
			raw: RawTypedKey::Proxy(NamedTypeId::of::<P>()),
		}
	}

	pub fn dynamic() -> Self {
		static ID_GEN: AtomicU64 = AtomicU64::new(0);

		let id = ID_GEN
			.fetch_update(Relaxed, Relaxed, |val| {
				Some(
					val.checked_add(1)
						.expect("cannot create more than u64::MAX dynamic keys!"),
				)
			})
			.unwrap();

		Self {
			_ty: PhantomData,
			raw: RawTypedKey::Runtime(id),
		}
	}
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum RawTypedKey {
	Instance(NamedTypeId),
	Proxy(NamedTypeId),
	Runtime(u64),
}

impl<T: ?Sized + 'static> From<TypedKey<T>> for RawTypedKey {
	fn from(key: TypedKey<T>) -> Self {
		key.raw()
	}
}

// === Provider === //

pub trait Provider {
	fn provide<'r>(&'r self, demand: &mut Demand<'r>);

	unsafe fn provide_single<'r>(&'r self, demand: &mut Demand<'r>) {
		match demand.kind() {
			DemandKind::FetchSingle(_) => {}
			// Safety: provided by caller
			_ => std::hint::unreachable_unchecked(),
		}
		self.provide(demand);
	}
}

#[repr(transparent)]
pub struct Demand<'r> {
	_ty: PhantomData<&'r ()>,
	erased: Incomplete<DemandKind>,
}

#[derive(Debug, Copy, Clone)]
#[non_exhaustive]
pub enum DemandKind {
	FetchSingle(RawTypedKey),
	EnumerateKeys,
	CompareLists,
}

#[repr(C)]
struct DemandSingle<'r, T: ?Sized> {
	kind: DemandKind,
	output: Option<&'r T>,
}

#[repr(C)]
struct DemandEnumerate {
	kind: DemandKind,
	collector: Vec<RawTypedKey>,
}

struct DemandCompare<'r> {
	#[allow(dead_code)] // False positive.
	kind: DemandKind,
	remaining: &'r [RawTypedKey],
	condemned: bool,
}

impl<'r> Demand<'r> {
	fn from_erased(erased: &mut Incomplete<DemandKind>) -> &mut Self {
		unsafe {
			// Safety: this type is repr(transparent) w.r.t. `Incomplete<DemandKind>`
			erased.cast_mut_via_ptr(|ptr| ptr as *mut Self)
		}
	}

	pub fn kind(&self) -> DemandKind {
		*self.erased
	}

	pub fn provide_in<T: ?Sized>(&mut self, key: TypedKey<T>, value: &'r T) -> &mut Self {
		match self.kind() {
			DemandKind::FetchSingle(desired_key) => {
				if key.raw() == desired_key {
					let demand =
						unsafe { Incomplete::cast_mut::<DemandSingle<'r, T>>(&mut self.erased) };

					debug_assert!(demand.output.is_none());
					demand.output = Some(value);
				}
			}
			DemandKind::EnumerateKeys => {
				let demand = unsafe { Incomplete::cast_mut::<DemandEnumerate>(&mut self.erased) };

				demand.collector.push(key.raw());
			}
			DemandKind::CompareLists => {
				let demand = unsafe { Incomplete::cast_mut::<DemandCompare>(&mut self.erased) };

				if demand
					.remaining
					.get(0)
					.map_or(false, |expected| key.raw() == *expected)
				{
					demand.remaining = &demand.remaining[1..];
				} else {
					demand.condemned = true;
				}
			}
		}
		self
	}

	pub fn propose<T: ?Sized + 'static>(&mut self, value: &'r T) -> &mut Self {
		self.provide_in(TypedKey::instance(), value)
	}
}

// === ProviderExt === //

#[derive_where(Clone)]
#[derive(Error)]
pub struct MissingComponentError<'a, P: ?Sized> {
	target: &'a P,
	request: RawTypedKey,
}

impl<P: ?Sized + Provider> fmt::Debug for MissingComponentError<'_, P> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("MissingComponentError")
			.field("request", &self.request)
			.finish_non_exhaustive()
	}
}

impl<P: ?Sized + Provider> fmt::Display for MissingComponentError<'_, P> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "failed to fetch component under key {:?}", self.request)?;

		let keys = self.target.keys();

		if keys.is_empty() {
			write!(f, "; provider has no entries.")?;
		} else {
			write!(
				f,
				"; provider exposes components: {}",
				keys.iter().map(|v| format!("{v:?}")).format(",")
			)?;
		}

		Ok(())
	}
}

pub trait ProviderExt: Provider {
	fn keys(&self) -> Vec<RawTypedKey> {
		let mut demand = DemandEnumerate {
			kind: DemandKind::EnumerateKeys,
			collector: Vec::new(),
		};

		let demand_erased = Incomplete::new_mut(&mut demand);
		let demand_erased = unsafe { Incomplete::cast_mut::<DemandKind>(demand_erased) };
		self.provide(Demand::from_erased(demand_erased));

		demand.collector
	}

	fn try_get_in<T: ?Sized + 'static>(
		&self,
		key: TypedKey<T>,
	) -> Result<&T, MissingComponentError<'_, Self>> {
		let mut demand = DemandSingle {
			kind: DemandKind::FetchSingle(key.raw()),
			output: None::<&T>,
		};
		let demand_erased = Incomplete::new_mut(&mut demand);
		let demand_erased = unsafe { Incomplete::cast_mut::<DemandKind>(demand_erased) };
		self.provide(Demand::from_erased(demand_erased));

		demand.output.ok_or(MissingComponentError {
			target: self,
			request: key.raw(),
		})
	}

	fn try_get<T: ?Sized + 'static>(&self) -> Result<&T, MissingComponentError<Self>> {
		self.try_get_in(TypedKey::instance())
	}

	fn has_in<T: ?Sized + 'static>(&self, key: TypedKey<T>) -> bool {
		self.try_get_in(key).is_ok()
	}

	fn has<T: ?Sized + 'static>(&self) -> bool {
		self.has_in(TypedKey::<T>::instance())
	}

	fn get_in<T: ?Sized + 'static>(&self, key: TypedKey<T>) -> &T {
		self.try_get_in(key).unwrap_pretty()
	}

	fn get<T: ?Sized + 'static>(&self) -> &T {
		self.get_in(TypedKey::instance())
	}

	fn borrow_in<'a, T: ?Sized + 'static>(
		&'a self,
		s: &'a impl Session,
		key: TypedKey<CompCell<T>>,
	) -> CompRef<'a, T> {
		self.get_in(key).borrow(s)
	}

	fn borrow<'a, T: ?Sized + 'static>(&'a self, s: &'a impl Session) -> CompRef<'a, T> {
		self.borrow_in(s, TypedKey::instance())
	}

	fn borrow_mut_in<'a, T: ?Sized + 'static>(
		&'a self,
		s: &'a impl Session,
		key: TypedKey<CompCell<T>>,
	) -> CompMut<'a, T> {
		self.get_in(key).borrow_mut(s)
	}

	fn borrow_mut<'a, T: ?Sized + 'static>(&'a self, s: &'a impl Session) -> CompMut<'a, T> {
		self.borrow_mut_in(s, TypedKey::instance())
	}
}

pub fn key_list_matches<T: ?Sized + Provider>(provider: &T, list: &[RawTypedKey]) -> bool {
	let mut demand = DemandCompare {
		kind: DemandKind::CompareLists,
		remaining: list,
		condemned: false,
	};

	let demand_erased = Incomplete::new_mut(&mut demand);
	let demand_erased = unsafe { Incomplete::cast_mut::<DemandKind>(demand_erased) };
	provider.provide(Demand::from_erased(demand_erased));

	!demand.condemned && demand.remaining.is_empty()
}

impl<P: ?Sized + Provider> ProviderExt for P {}

// === Basic Providers === //

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd, Default)]
pub struct EmptyProvider;

impl Provider for EmptyProvider {
	fn provide<'r>(&'r self, _demand: &mut Demand<'r>) {}
}

// === Archetypal === //

pub struct Entity<T: ?Sized + Provider = dyn Send + Sync + Provider> {
	_archetype: (),
	provider: T,
}

impl<T: ?Sized + Provider> fmt::Debug for Entity<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("Entity").finish_non_exhaustive()
	}
}

impl<T: ?Sized + Provider> Entity<T> {
	pub fn new(provider: T) -> Self
	where
		T: Sized,
	{
		Self {
			_archetype: (),
			provider,
		}
	}

	pub fn new_arc(provider: T) -> Arc<Entity>
	where
		T: Sized + Send + Sync + 'static,
	{
		Arc::new(Self::new(provider))
	}
}

impl<T: ?Sized + Provider> Provider for Entity<T> {
	fn provide<'r>(&'r self, demand: &mut Demand<'r>) {
		// TODO: Fast-path archetypal components
		self.provider.provide(demand);
	}
}

// === Tests === //

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn static_example() {
		struct Foo {
			a: u32,
			b: i32,
			c: String,
		}

		impl Provider for Foo {
			fn provide<'r>(&'r self, demand: &mut Demand<'r>) {
				demand
					.propose(&self.a)
					.propose(&self.b)
					.propose::<String>(&self.c)
					.propose::<str>(&self.c);
			}
		}

		let foo = Foo {
			a: 3,
			b: 4,
			c: "foo".to_string(),
		};

		let bar = &foo as &dyn Provider;

		assert_eq!(*bar.get::<u32>(), 3);
		assert_eq!(*bar.get::<i32>(), 4);
		assert_eq!(bar.get::<String>(), "foo");
		assert_eq!(bar.get::<str>(), "foo");

		let foo = Entity::new(foo);

		assert_eq!(*foo.get::<u32>(), 3);
		assert_eq!(*foo.get::<i32>(), 4);
		assert_eq!(foo.get::<String>(), "foo");
		assert_eq!(foo.get::<str>(), "foo");
	}
}
