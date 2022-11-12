use std::{
	any::type_name,
	cell::{Ref, RefCell, RefMut},
	fmt::Debug,
};

use derive_where::derive_where;

use crate::{lang::sync::ExtRefCell, mem::ptr::PointeeCastExt};

use super::core::{failed_to_find_component, Entity, Storage};

#[derive(Debug)]
#[derive_where(Default)]
#[repr(transparent)]
pub struct CelledStorage<T> {
	inner: Storage<ExtRefCell<T>>,
}

impl<T> CelledStorage<T> {
	pub fn new() -> Self {
		Self {
			inner: Storage::new(),
		}
	}

	pub fn add(&mut self, entity: Entity, value: T) -> (Option<T>, &mut T) {
		let (replaced, current) = self.inner.add(entity, ExtRefCell::new(value));

		(replaced.map(ExtRefCell::into_inner), current)
	}

	pub fn remove(&mut self, entity: Entity) -> Option<T> {
		self.inner.remove(entity).map(ExtRefCell::into_inner)
	}

	pub fn remove_many<I>(&mut self, entities: I)
	where
		I: IntoIterator<Item = Entity>,
	{
		self.inner.remove_many(entities);
	}

	pub fn try_get(&self, entity: Entity) -> Option<&T> {
		self.inner.try_get(entity).map(|v| &**v)
	}

	pub fn try_get_mut(&mut self, entity: Entity) -> Option<&mut T> {
		self.inner.try_get_mut(entity).map(|v| &mut **v)
	}

	pub fn get(&self, entity: Entity) -> &T {
		self.try_get(entity).unwrap_or_else(|| {
			panic!(
				"failed to find entity {entity:?} with component {}",
				type_name::<T>()
			)
		})
	}

	pub fn get_mut(&mut self, entity: Entity) -> &mut T {
		self.try_get_mut(entity).unwrap_or_else(|| {
			panic!(
				"failed to find entity {entity:?} with component {}",
				type_name::<T>()
			)
		})
	}

	pub fn clear(&mut self) {
		self.inner.clear();
	}

	pub fn borrow_dyn(&mut self) -> &mut CelledStorageView<T> {
		unsafe {
			// FIXME: Reconsider transmute safety, especially as it relates to `Storage<ExtRefCell<T>>`
			// to `Storage<RefCell<T>>` conversion—`HashMap` doesn't officially guarantee the same
			// transmute properties as `Vec<T>`. This will hopefully resolve itself as soon as we
			// write a `PerfectHashMap`.
			//
			// As for logical soundness, we only expose the underlying `RefCell` when we know that we
			// have exclusive access of the underlying container. This corresponds roughly to a
			// `&mut T` to `&mut Cell<T>` conversion in terms of soundness semantics.
			self.cast_mut_via_ptr(|p| p as *mut CelledStorageView<T>)
		}
	}
}

impl<T: Clone> Clone for CelledStorage<T> {
	fn clone(&self) -> Self {
		Self {
			inner: self.inner.clone(),
		}
	}
}

#[derive(Debug)]
#[repr(transparent)]
pub struct CelledStorageView<T> {
	inner: Storage<RefCell<T>>,
}

impl<T> CelledStorageView<T> {
	pub fn add(&mut self, entity: Entity, value: T) -> (Option<T>, &mut T) {
		let (replaced, current) = self.inner.add(entity, RefCell::new(value));

		(replaced.map(RefCell::into_inner), current.get_mut())
	}

	pub fn remove(&mut self, entity: Entity) -> Option<T> {
		self.inner.remove(entity).map(RefCell::into_inner)
	}

	pub fn remove_many<I>(&mut self, entities: I)
	where
		I: IntoIterator<Item = Entity>,
	{
		self.inner.remove_many(entities);
	}

	pub fn try_get_cell(&self, entity: Entity) -> Option<&RefCell<T>> {
		self.inner.try_get(entity)
	}

	pub fn try_get_cell_mut(&mut self, entity: Entity) -> Option<&mut RefCell<T>> {
		self.inner.try_get_mut(entity)
	}

	pub fn get_cell(&self, entity: Entity) -> &RefCell<T> {
		self.try_get_cell(entity)
			.unwrap_or_else(|| failed_to_find_component::<T>(entity))
	}

	pub fn get_cell_mut(&mut self, entity: Entity) -> &mut RefCell<T> {
		self.try_get_cell_mut(entity)
			.unwrap_or_else(|| failed_to_find_component::<T>(entity))
	}

	pub fn try_get_mut(&mut self, entity: Entity) -> Option<&mut T> {
		self.try_get_cell_mut(entity).map(RefCell::get_mut)
	}

	pub fn get_mut(&mut self, entity: Entity) -> &mut T {
		self.try_get_mut(entity)
			.unwrap_or_else(|| failed_to_find_component::<T>(entity))
	}

	pub fn borrow(&self, entity: Entity) -> Ref<T> {
		self.get_cell(entity).borrow()
	}

	pub fn borrow_mut(&self, entity: Entity) -> RefMut<T> {
		self.get_cell(entity).borrow_mut()
	}

	pub fn clear(&mut self) {
		self.inner.clear();
	}
}
