use std::{
	any::{type_name, Any},
	mem,
	ops::{Deref, DerefMut},
};

use derive_where::derive_where;
use hashbrown::HashMap;
use parking_lot::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::{
	debug::{
		type_id::NamedTypeId,
		userdata::{ErasedUserdataValue, UserdataValue},
	},
	lang::std_traits::Mutability,
	mem::ptr::{runtime_unify_mut, runtime_unify_ref, PointeeCastExt},
};

use super::provider::{DynProvider, Provider, ProviderPackPart};

// === Universe === //

#[derive(Debug)]
pub struct Universe(UniverseHandle);

impl Universe {
	pub fn new() -> Self {
		Self(UniverseHandle {
			established: Default::default(),
			new: Default::default(),
		})
	}

	pub fn handle(&self) -> &UniverseHandle {
		&self.0
	}

	pub fn flush(&mut self) {
		self.0.established.extend(
			mem::replace(self.0.new.get_mut(), HashMap::new())
				.into_iter()
				.map(|(k, v)| (k, v.unwrap())),
		)
	}
}

// TODO
// impl Provider for Universe {
// 	fn build_dyn_provider<'r>(&'r mut self, _provider: &mut DynProvider<'r>) {
// 		unimplemented!()
// 	}
//
// 	unsafe fn try_get_comp_unchecked<'a, U: ?Sized + 'static>(me: *const Self) -> Option<&'a U>
// 	where
// 		Self: 'a,
// 	{
// 		let me = &*me;
// 		me.0.try_acquire::<U>()
// 	}
//
// 	unsafe fn try_get_comp_mut_unchecked<'a, U: ?Sized + 'static>(
// 		me: *mut Self,
// 	) -> Option<&'a mut U>
// 	where
// 		Self: 'a,
// 	{
// 		todo!()
// 	}
// }

impl Default for Universe {
	fn default() -> Self {
		Self::new()
	}
}

impl Deref for Universe {
	type Target = UniverseHandle;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

#[derive(Debug)]
pub struct UniverseHandle {
	established: HashMap<NamedTypeId, Box<dyn UserdataValue>>,
	new: Mutex<HashMap<NamedTypeId, Option<Box<dyn UserdataValue>>>>,
}

impl UniverseHandle {
	pub fn try_acquire<T: Any>(&self) -> Option<&T> {
		let key = NamedTypeId::of::<T>();

		if let Some(established) = self.established.get(&key) {
			return Some(established.downcast_ref());
		}

		self.new.lock().get(&key).map(|inner| {
			let inner = inner.as_ref().unwrap_or_else(|| {
				panic!("Attempted to acquire auto value {key:?} while it was being initialized.")
			});

			let inner = unsafe {
				// Safety: as long as `UniverseHandle` is borrowed, components will not be flushed
				// from the `new` map or accessed mutably.
				(&*inner).prolong()
			};

			inner.downcast_ref()
		})
	}

	pub fn acquire_or_create<T, F>(&self, f: F) -> &T
	where
		T: UserdataValue,
		F: FnOnce() -> T,
	{
		let key = NamedTypeId::of::<T>();

		// See if we can acquire the component directly from the established map.
		if let Some(established) = self.established.get(&key) {
			return established.downcast_ref();
		}

		// Otherwise, see if it's a new component and acquire it from there.
		if let Some(established) = self.new.lock().entry(key).or_insert(None) {
			let established = unsafe {
				// Safety: as long as `UniverseHandle` is borrowed, components will not be flushed
				// from the `new` map or accessed mutably.
				(&*established).prolong()
			};
			return established.downcast_ref();
		}

		// If both checks failed, create the value...
		let value = Box::new(f());
		let inner = unsafe {
			// Safety: as long as `UniverseHandle` is borrowed, components will not be flushed
			// from the `new` map or accessed mutably.
			(&*value).prolong()
		};

		// ...and put it in the new map.
		// N.B. because we're transferring control to user code, we have to reborrow this lock.
		self.new.lock().insert(key, Some(value));

		inner
	}
}

pub trait AutoValue: UserdataValue {
	fn create(universe: &UniverseHandle) -> Self;
}

// === Auto === //

fn auto_acquire_failed<T: AutoValue>(mode: Mutability) -> ! {
	panic!(
		"Attempted to acquire universe component {:?} {} but it was already locked {} elsewhere.",
		type_name::<T>(),
		mode.adverb(),
		mode.inverse().adverb()
	);
}

#[derive(Debug)]
#[derive_where(Copy, Clone)]
pub struct Auto<'a, T>(pub &'a T);

impl<'a, T> Deref for Auto<'a, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&*self.0
	}
}

impl<T: AutoValue> Provider for Auto<'_, T> {
	fn build_dyn_provider<'r>(&'r mut self, provider: &mut DynProvider<'r>) {
		provider.add_ref::<T>(&*self);
	}

	unsafe fn try_get_comp_unchecked<'a, U: ?Sized + 'static>(me: *const Self) -> Option<&'a U>
	where
		Self: 'a,
	{
		if NamedTypeId::of::<T>() == NamedTypeId::of::<U>() {
			let me: &'a Self = &*me;
			Some(runtime_unify_ref::<T, U>(&**me))
		} else {
			None
		}
	}

	unsafe fn try_get_comp_mut_unchecked<'a, U: ?Sized + 'static>(
		_me: *mut Self,
	) -> Option<&'a mut U>
	where
		Self: 'a,
	{
		None
	}
}

impl<'a, T: AutoValue> ProviderPackPart<'a> for Auto<'a, T> {
	type AliasPointee = T;

	unsafe fn pack_from<Q: 'a + Provider>(provider: *mut Q) -> Self {
		// Try to get a reference from the provider itself.
		if let Some(value) = Q::try_get_comp_unchecked(provider) {
			return Self(value);
		}

		// Otherwise, get it from the universe.
		// Safety: `UniverseHandle` is never exposed mutably because outside code can't even create a
		// mutable reference to this type.
		let universe = Q::try_get_comp_unchecked::<UniverseHandle>(provider)
			.expect("attempted to fetch an `Auto` without providing a `Universe` to do so.");

		Self(universe.acquire_or_create(|| T::create(universe)))
	}
}

// === AutoRef === //

#[derive(Debug)]
pub enum AutoRef<'a, T: AutoValue> {
	Mutexed(RwLockReadGuard<'a, T>),
	Ref(&'a T),
}

impl<T: AutoValue> Deref for AutoRef<'_, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		match self {
			Self::Mutexed(guard) => &guard,
			Self::Ref(value) => &value,
		}
	}
}

impl<T: AutoValue> Provider for AutoRef<'_, T> {
	fn build_dyn_provider<'r>(&'r mut self, provider: &mut DynProvider<'r>) {
		provider.add_ref::<T>(&*self);
	}

	unsafe fn try_get_comp_unchecked<'a, U: ?Sized + 'static>(me: *const Self) -> Option<&'a U>
	where
		Self: 'a,
	{
		if NamedTypeId::of::<T>() == NamedTypeId::of::<U>() {
			let me: &'a Self = &*me;
			Some(runtime_unify_ref::<T, U>(&**me))
		} else {
			None
		}
	}

	unsafe fn try_get_comp_mut_unchecked<'a, U: ?Sized + 'static>(
		_me: *mut Self,
	) -> Option<&'a mut U>
	where
		Self: 'a,
	{
		None
	}
}

impl<'a, T: AutoValue> ProviderPackPart<'a> for AutoRef<'a, T> {
	type AliasPointee = T;

	unsafe fn pack_from<Q: 'a + Provider>(provider: *mut Q) -> Self {
		// Try to get a reference from the provider itself.
		if let Some(value) = Q::try_get_comp_unchecked(provider) {
			return Self::Ref(value);
		}

		// Otherwise, borrow from the universe.
		// Safety: `UniverseHandle` is never exposed mutably because outside code can't even create a
		// mutable reference to this type.
		let universe = Q::try_get_comp_unchecked::<UniverseHandle>(provider)
			.expect("attempted to fetch an `AutoRef` without providing a `Universe` to do so.");

		Self::Mutexed(
			universe
				.acquire_or_create::<RwLock<T>, _>(|| RwLock::new(T::create(universe)))
				.try_read()
				.unwrap_or_else(|| auto_acquire_failed::<T>(Mutability::Immutable)),
		)
	}
}

// === AutoMut === //

#[derive(Debug)]
pub enum AutoMut<'a, T: AutoValue> {
	Mutexed(RwLockWriteGuard<'a, T>),
	Ref(&'a mut T),
}

impl<T: AutoValue> Deref for AutoMut<'_, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		match self {
			Self::Mutexed(guard) => &guard,
			Self::Ref(value) => &value,
		}
	}
}

impl<T: AutoValue> DerefMut for AutoMut<'_, T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		match self {
			Self::Mutexed(guard) => &mut *guard,
			Self::Ref(value) => &mut *value,
		}
	}
}

impl<T: AutoValue> Provider for AutoMut<'_, T> {
	fn build_dyn_provider<'r>(&'r mut self, provider: &mut DynProvider<'r>) {
		provider.add_ref::<T>(&*self);
	}

	unsafe fn try_get_comp_unchecked<'a, U: ?Sized + 'static>(me: *const Self) -> Option<&'a U>
	where
		Self: 'a,
	{
		if NamedTypeId::of::<T>() == NamedTypeId::of::<U>() {
			let me: &'a Self = &*me;
			Some(runtime_unify_ref::<T, U>(&**me))
		} else {
			None
		}
	}

	unsafe fn try_get_comp_mut_unchecked<'a, U: ?Sized + 'static>(
		me: *mut Self,
	) -> Option<&'a mut U>
	where
		Self: 'a,
	{
		if NamedTypeId::of::<T>() == NamedTypeId::of::<U>() {
			let me: &'a mut Self = &mut *me;
			Some(runtime_unify_mut::<T, U>(&mut **me))
		} else {
			None
		}
	}
}

impl<'a, T: AutoValue> ProviderPackPart<'a> for AutoMut<'a, T> {
	type AliasPointee = T;

	unsafe fn pack_from<Q: 'a + Provider>(provider: *mut Q) -> Self {
		// Try to get a reference from the provider itself.
		if let Some(value) = Q::try_get_comp_mut_unchecked(provider) {
			return Self::Ref(value);
		}

		// Otherwise, borrow from the universe.
		// Safety: `UniverseHandle` is never exposed mutably because outside code can't even create a
		// mutable reference to this type.
		let universe = Q::try_get_comp_unchecked::<UniverseHandle>(provider)
			.expect("attempted to fetch an `AutoMut` without providing a `Universe` to do so.");

		Self::Mutexed(
			universe
				.acquire_or_create::<RwLock<T>, _>(|| RwLock::new(T::create(universe)))
				.try_write()
				.unwrap_or_else(|| auto_acquire_failed::<T>(Mutability::Mutable)),
		)
	}
}
