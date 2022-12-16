use std::{any::type_name, mem};

use hashbrown::HashMap;
use parking_lot::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::{
	debug::{
		type_id::NamedTypeId,
		userdata::{ErasedUserdataValue, Userdata, UserdataValue},
	},
	mem::ptr::{sizealign_checked_transmute, PointeeCastExt},
};

use super::provider::{DynProvider, Provider};

#[derive(Debug, Default)]
pub struct TypeContainer {
	established: HashMap<NamedTypeId, Userdata>,
	new: Mutex<HashMap<NamedTypeId, Option<Userdata>>>,
}

impl TypeContainer {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn get_raw<T: ?Sized + 'static>(&self) -> Option<*mut T> {
		let key = NamedTypeId::of::<T>();

		if let Some(established) = self.established.get(&key) {
			let ptr = &**established as *const dyn UserdataValue as *mut ();
			return Some(unsafe {
				// Safety: all components in this map are `Sized`.
				sizealign_checked_transmute(ptr)
			});
		}

		self.new.lock().get(&key).map(|inner| {
			let inner = inner.as_ref().unwrap_or_else(|| {
				panic!("Attempted to acquire auto value {key:?} while it was being initialized.")
			});

			let ptr = &**inner as *const dyn UserdataValue as *mut ();

			unsafe {
				// Safety: all components in this map are `Sized`.
				sizealign_checked_transmute(ptr)
			}
		})
	}

	pub fn get_or_create<T, F>(&self, f: F) -> &T
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

	pub fn lock_ref<T: UserdataValue>(&self) -> RwLockReadGuard<T> {
		self.get_comp::<RwLock<T>>().try_read().unwrap_or_else(|| {
			panic!(
				"failed to acquire component {:?} immutably.",
				type_name::<RwLock<T>>()
			)
		})
	}

	pub fn lock_mut<T: UserdataValue>(&self) -> RwLockWriteGuard<T> {
		self.get_comp::<RwLock<T>>().try_write().unwrap_or_else(|| {
			panic!(
				"failed to acquire component {:?} mutably.",
				type_name::<RwLock<T>>()
			)
		})
	}

	pub fn lock_ref_or_create<T: UserdataValue, F>(&self, f: F) -> RwLockReadGuard<T>
	where
		F: FnOnce() -> T,
	{
		self.get_or_create::<RwLock<T>, _>(|| RwLock::new(f()))
			.try_read()
			.unwrap()
	}

	pub fn lock_mut_or_create<T: UserdataValue, F>(&self, f: F) -> RwLockWriteGuard<T>
	where
		F: FnOnce() -> T,
	{
		self.get_or_create::<RwLock<T>, _>(|| RwLock::new(f()))
			.try_write()
			.unwrap()
	}

	pub fn remove<T: UserdataValue>(&mut self) -> Option<T> {
		self.flush();
		todo!()
	}

	pub fn flush(&mut self) {
		self.established.extend(
			mem::replace(self.new.get_mut(), HashMap::new())
				.into_iter()
				.map(|(k, v)| (k, v.unwrap())),
		)
	}
}

impl Provider for TypeContainer {
	fn build_dyn_provider<'r>(&'r mut self, _provider: &mut DynProvider<'r>) {
		unimplemented!() // TODO
	}

	unsafe fn try_get_comp_unchecked<'a, U: ?Sized + 'static>(me: *const Self) -> Option<&'a U>
	where
		Self: 'a,
	{
		// Safety: when used as a provider, we never acquire this object mutably.
		let me = &*me;

		me.get_raw::<U>().map(|p| unsafe { &*p })
	}

	unsafe fn try_get_comp_mut_unchecked<'a, U: ?Sized + 'static>(
		me: *mut Self,
	) -> Option<&'a mut U>
	where
		Self: 'a,
	{
		// Safety: when used as a provider, we never acquire this object mutably.
		let me = &*me;

		me.get_raw::<U>().map(|p| unsafe { &mut *p })
	}
}
