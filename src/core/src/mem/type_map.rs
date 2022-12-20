use std::any::type_name;

use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::{
	debug::{
		type_id::NamedTypeId,
		userdata::{ErasedUserdata, Userdata},
	},
	lang::loan::downcast_userdata_box,
};

use super::{drop_guard::DropOwned, eventual_map::EventualMap};

#[derive(Debug, Default)]
pub struct TypeMap {
	map: EventualMap<NamedTypeId, dyn Userdata>,
}

impl TypeMap {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn get_or_create<T, F>(&self, f: F) -> &T
	where
		T: Userdata,
		F: FnOnce() -> T,
	{
		self.map
			.get_or_create(NamedTypeId::of::<T>(), || Box::new(f()))
			.downcast_ref()
	}

	pub fn try_get_comp<T: Userdata>(&self) -> Option<&T> {
		self.map
			.get(&NamedTypeId::of::<T>())
			.map(|v| v.downcast_ref())
	}

	pub fn get_comp<T: Userdata>(&self) -> &T {
		self.try_get_comp().unwrap_or_else(|| {
			panic!(
				"failed to get component of type {:?} in `TypeMap`.",
				type_name::<T>()
			)
		})
	}

	pub fn lock_ref<T: Userdata>(&self) -> RwLockReadGuard<T> {
		self.get_comp::<RwLock<T>>().try_read().unwrap_or_else(|| {
			panic!(
				"failed to acquire component {:?} immutably.",
				type_name::<RwLock<T>>()
			)
		})
	}

	pub fn lock_mut<T: Userdata>(&self) -> RwLockWriteGuard<T> {
		self.get_comp::<RwLock<T>>().try_write().unwrap_or_else(|| {
			panic!(
				"failed to acquire component {:?} mutably.",
				type_name::<RwLock<T>>()
			)
		})
	}

	pub fn lock_ref_or_create<T: Userdata, F>(&self, f: F) -> RwLockReadGuard<T>
	where
		F: FnOnce() -> T,
	{
		self.get_or_create::<RwLock<T>, _>(|| RwLock::new(f()))
			.try_read()
			.unwrap()
	}

	pub fn lock_mut_or_create<T: Userdata, F>(&self, f: F) -> RwLockWriteGuard<T>
	where
		F: FnOnce() -> T,
	{
		self.get_or_create::<RwLock<T>, _>(|| RwLock::new(f()))
			.try_write()
			.unwrap()
	}

	pub fn remove<T: Userdata>(&mut self) -> Option<Box<T>> {
		self.map
			.remove(&NamedTypeId::of::<T>())
			.map(downcast_userdata_box)
	}

	pub fn remove_and_drop<T: Userdata + DropOwned<C>, C>(&mut self, cx: C) {
		if let Some(target) = self.remove::<T>() {
			target.drop_owned(cx);
		}
	}

	pub fn flush(&mut self) {
		self.map.flush();
	}
}

// TODO: Implement `Provider`
