use std::{ops::Deref, sync::Arc};

use hashbrown::HashMap;
use parking_lot::{Mutex, RwLock};

use crate::{
	debug::{type_id::NamedTypeId, userdata::UserdataValue},
	lang::loan::{downcast_userdata_arc, ArcReadGuard},
	mem::ptr::runtime_unify_ref,
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
		todo!();
	}
}

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
	established: HashMap<NamedTypeId, Arc<dyn UserdataValue>>,
	new: Mutex<HashMap<NamedTypeId, Option<Arc<dyn UserdataValue>>>>,
}

impl UniverseHandle {
	pub fn try_acquire<T: UserdataValue>(&self) -> Option<Arc<RwLock<T>>> {
		let key = NamedTypeId::of::<T>();

		if let Some(established) = self.established.get(&key) {
			return Some(downcast_userdata_arc(established.clone()));
		}

		let arc = self.new.lock().get(&key).cloned();

		arc.map(|inner| {
			let inner = inner.unwrap_or_else(|| {
				panic!("Attempted to acquire auto value {key:?} while it was being initialized.")
			});
			downcast_userdata_arc(inner)
		})
	}

	pub fn acquire_or_create<T, F>(&self, f: F) -> Arc<RwLock<T>>
	where
		T: UserdataValue,
		F: FnOnce() -> T,
	{
		let key = NamedTypeId::of::<T>();

		if let Some(established) = self.established.get(&key) {
			return downcast_userdata_arc(established.clone());
		}

		let new = self.new.lock().entry(key).or_insert(None).clone();
		if let Some(established) = new {
			return downcast_userdata_arc(established);
		}

		let value = Arc::new(RwLock::new(f()));
		let value_clone = value.clone();
		self.new.lock().insert(key, Some(value_clone));
		value
	}
}

pub trait AutoValue: Default + UserdataValue {}

// === AutoStorage === //

#[derive(Debug)]
pub enum AutoRef<'a, T: AutoValue> {
	Mutexed(ArcReadGuard<T>),
	Ref(&'a T),
}

impl<T: AutoValue> Deref for AutoRef<'_, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		match self {
			AutoRef::Mutexed(guard) => &guard,
			AutoRef::Ref(value) => &value,
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
			let comp = match me {
				AutoRef::Mutexed(guard) => &**guard,
				AutoRef::Ref(value) => &value,
			};
			let comp = runtime_unify_ref::<T, U>(comp);

			Some(comp)
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
			return AutoRef::Ref(value);
		}

		// Otherwise, borrow from the universe.
		// Safety: `UniverseHandle` is never exposed mutably because outside code can't even create a
		// mutable reference to this type.
		let universe = Q::try_get_comp_unchecked::<UniverseHandle>(provider)
			.expect("attempted to fetch a `AutoRef` without providing a `Universe` to do so.");

		let arc = universe.acquire_or_create::<T, _>(|| Default::default());
		let arc = ArcReadGuard::try_new(arc).unwrap();

		AutoRef::Mutexed(arc)
	}
}
