use std::{
	any::type_name,
	cell::{Ref, RefCell, RefMut},
	fmt::Debug,
};

use derive_where::derive_where;

use crate::{
	lang::{std_traits::UnsafeCellLike, sync::AssertSync},
	mem::ptr::PointeeCastExt,
};

use super::core::{failed_to_find_component, Entity, Storage};

struct MyCell<T>(AssertSync<RefCell<T>>);

impl<T: Debug> Debug for MyCell<T> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		unsafe {
			// Safety: `RefCell` state can only be modified so long as the calling thread proves that
			// they have mutable reference to this container. Calls like these are therefore safe.
			self.0.get()
		}
		.fmt(f)
	}
}

impl<T: Clone> Clone for MyCell<T> {
	fn clone(&self) -> Self {
		Self(AssertSync::new(
			unsafe {
				// Safety: `RefCell` state can only be modified so long as the calling thread proves
				// that they have mutable reference to this container. Calls like these are therefore
				// safe.
				self.0.get()
			}
			.clone(),
		))
	}
}

#[derive(Debug)]
#[derive_where(Default)]
pub struct CelledStorage<T> {
	inner: Storage<MyCell<T>>,
}

impl<T> CelledStorage<T> {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn add(&mut self, entity: Entity, value: T) -> (Option<T>, &mut T) {
		let (replaced, r) = self
			.inner
			.add(entity, MyCell(AssertSync::new(RefCell::new(value))));

		(
			replaced.map(|r| r.0.into_inner().into_inner()),
			r.0.get_mut().get_mut(),
		)
	}

	pub fn remove(&mut self, entity: Entity) -> Option<T> {
		self.inner
			.remove(entity)
			.map(|v| v.0.into_inner().into_inner())
	}

	pub fn remove_many<I>(&mut self, entities: I)
	where
		I: IntoIterator<Item = Entity>,
	{
		self.inner.remove_many(entities);
	}

	pub fn try_get(&self, entity: Entity) -> Option<&T> {
		self.inner.try_get(entity).map(|v| unsafe {
			// Safety: when interacting with just a `CelledStorage`, we expose regular borrowing
			// semantics. `RefCell`'ed semantics are only exposed with `borrow_dyn`, which requires
			// a mutable reference to this container.
			v.0.get().get_ref_unchecked()
		})
	}

	pub fn try_get_mut(&mut self, entity: Entity) -> Option<&mut T> {
		self.inner
			.try_get_mut(entity)
			.map(|v| v.0.get_mut().get_mut())
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
		unsafe { self.cast_mut_via_ptr(|p| p as *mut CelledStorageView<T>) }
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
pub struct CelledStorageView<T>(CelledStorage<T>);

impl<T> CelledStorageView<T> {
	pub fn add(&mut self, entity: Entity, value: T) -> (Option<T>, &mut T) {
		self.0.add(entity, value)
	}

	pub fn remove(&mut self, entity: Entity) -> Option<T> {
		self.0.remove(entity)
	}

	pub fn remove_many<I>(&mut self, entities: I)
	where
		I: IntoIterator<Item = Entity>,
	{
		self.0.remove_many(entities);
	}

	pub fn try_get_cell(&self, entity: Entity) -> Option<&RefCell<T>> {
		self.0.inner.try_get(entity).map(|v| unsafe {
			// Safety: the calling thread has exclusive access to these `RefCells`, making this safe.
			v.0.get()
		})
	}

	pub fn try_get_cell_mut(&mut self, entity: Entity) -> Option<&mut RefCell<T>> {
		self.0.inner.try_get_mut(entity).map(|v| v.0.get_mut())
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
		self.try_get_cell_mut(entity).map(|v| v.get_mut())
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
		self.0.clear();
	}
}
