use std::{
	any::type_name,
	cell::{Ref, RefCell, RefMut},
	fmt::Debug,
	num::NonZeroU32,
	ops::{self, Deref, DerefMut},
};

use derive_where::derive_where;

use crate::{
	debug::lifetime::{DebugLifetime, Dependent},
	lang::{polyfill::VecPoly, sync::ExtRefCell},
	mem::{
		auto_map::{AutoHashMap, AutoMut, CanForget, DefaultForgetPolicy},
		ptr::PointeeCastExt,
	},
};

use super::{
	entity::{ArchetypeId, Entity},
	query::{Query, QueryIter},
};

// === Storage === //

#[derive(Debug, Clone)]
#[derive_where(Default)]
#[repr(transparent)]
pub struct Storage<T> {
	archetypes: AutoHashMap<NonZeroU32, StorageRun<T>>,
}

impl<T> Storage<T> {
	pub fn new() -> Self {
		Self {
			archetypes: AutoHashMap::new(),
		}
	}

	pub fn get_run(&self, archetype: ArchetypeId) -> Option<&StorageRun<T>> {
		assert!(archetype.lifetime.is_possibly_alive());

		self.archetypes.get(&archetype.id)
	}

	pub fn get_run_mut(&mut self, archetype: ArchetypeId) -> Option<StorageRunRefMut<T>> {
		assert!(archetype.lifetime.is_possibly_alive());

		self.archetypes.get_mut(&archetype.id).map(StorageRunRefMut)
	}

	pub fn get_run_slice(&self, archetype: ArchetypeId) -> &[Option<StorageRunSlot<T>>] {
		match self.get_run(archetype) {
			Some(run) => run.as_slice(),
			None => &[],
		}
	}

	pub fn get_run_slice_mut(
		&mut self,
		archetype: ArchetypeId,
	) -> &mut [Option<StorageRunSlot<T>>] {
		match self.get_run_mut(archetype) {
			Some(run) => run.defuse_auto_removal().as_mut_slice(),
			None => &mut [],
		}
	}

	pub fn get_or_create_run(&mut self, archetype: ArchetypeId) -> StorageRunRefMut<T> {
		assert!(archetype.lifetime.is_possibly_alive());

		let run = self
			.archetypes
			.get_or_insert_with(archetype.id, || Default::default());

		StorageRunRefMut(run)
	}

	pub fn add(&mut self, entity: Entity, value: T) -> (Option<T>, &mut T) {
		self.get_or_create_run(entity.arch)
			.defuse_auto_removal()
			.add(entity, value)
	}

	pub fn remove(&mut self, entity: Entity) -> Option<T> {
		self.archetypes
			.get_mut(&entity.arch.id)?
			.remove(entity.slot)
	}

	pub fn remove_many<I>(&mut self, entities: I)
	where
		I: IntoIterator<Item = Entity>,
	{
		for entity in entities {
			self.remove(entity);
		}
	}

	pub fn get(&self, entity: Entity) -> Option<&T> {
		assert!(entity.lifetime.is_possibly_alive());

		self.archetypes
			.get(&entity.arch.id)?
			.get(entity.slot)
			.map(StorageRunSlot::value)
	}

	pub fn get_mut(&mut self, entity: Entity) -> Option<&mut T> {
		assert!(entity.lifetime.is_possibly_alive());

		AutoMut::defuse(self.archetypes.get_mut(&entity.arch.id)?)
			.get_mut(entity.slot)
			.map(StorageRunSlot::value_mut)
	}

	pub fn clear(&mut self) {
		self.archetypes.clear();
	}

	pub fn query_in_ref(&self, archetype: ArchetypeId) -> QueryIter<(&StorageRunSlice<T>,)> {
		(self,).query_in(archetype)
	}

	pub fn query_in_mut(
		&mut self,
		archetype: ArchetypeId,
	) -> QueryIter<(&mut StorageRunSlice<T>,)> {
		(self,).query_in(archetype)
	}
}

impl<T> ops::Index<Entity> for Storage<T> {
	type Output = T;

	fn index(&self, entity: Entity) -> &Self::Output {
		self.get(entity)
			.unwrap_or_else(|| failed_to_find_component::<T>(entity))
	}
}

impl<T> ops::IndexMut<Entity> for Storage<T> {
	fn index_mut(&mut self, entity: Entity) -> &mut Self::Output {
		self.get_mut(entity)
			.unwrap_or_else(|| failed_to_find_component::<T>(entity))
	}
}

pub(super) fn failed_to_find_component<T>(entity: Entity) -> ! {
	panic!(
		"failed to find entity {entity:?} with component {}",
		type_name::<T>()
	);
}

pub type StorageRunSlice<T> = [Option<StorageRunSlot<T>>];

#[derive(Debug, Clone)]
#[derive_where(Default)]
pub struct StorageRun<T> {
	comps: Vec<Option<StorageRunSlot<T>>>,
}

impl<T> StorageRun<T> {
	pub fn new() -> Self {
		Self { comps: Vec::new() }
	}

	pub fn add(&mut self, entity: Entity, value: T) -> (Option<T>, &mut T) {
		let slot = self.comps.ensure_slot_with(entity.slot_usize(), || None);
		let replaced = slot
			.replace(StorageRunSlot {
				lifetime: Dependent::new(entity.lifetime),
				value,
			})
			.map(|v| v.value);

		(replaced, slot.as_mut().unwrap().value_mut())
	}

	pub fn remove(&mut self, slot: u32) -> Option<T> {
		let removed = self.comps[slot as usize].take().map(|v| v.value);

		while matches!(self.comps.last(), Some(None)) {
			self.comps.pop();
		}

		removed
	}

	pub fn get(&self, slot: u32) -> Option<&StorageRunSlot<T>> {
		self.comps.get(slot as usize).and_then(|opt| opt.as_ref())
	}

	pub fn get_mut(&mut self, slot: u32) -> Option<&mut StorageRunSlot<T>> {
		self.comps
			.get_mut(slot as usize)
			.and_then(|opt| opt.as_mut())
	}

	pub fn max_slot(&self) -> u32 {
		self.comps.len() as u32
	}

	pub fn as_slice(&self) -> &StorageRunSlice<T> {
		self.comps.as_slice()
	}

	pub fn as_mut_slice(&mut self) -> &mut StorageRunSlice<T> {
		self.comps.as_mut_slice()
	}
}

impl<T> CanForget for StorageRun<T> {
	fn is_alive(&self) -> bool {
		!self.comps.is_empty()
	}
}

pub struct StorageRunRefMut<'a, T>(AutoMut<'a, NonZeroU32, StorageRun<T>, DefaultForgetPolicy>);

impl<'a, T> StorageRunRefMut<'a, T> {
	pub fn defuse_auto_removal(self) -> &'a mut StorageRun<T> {
		AutoMut::defuse(self.0)
	}
}

impl<T> Deref for StorageRunRefMut<'_, T> {
	type Target = StorageRun<T>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl<T> DerefMut for StorageRunRefMut<'_, T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

#[derive(Debug, Clone)]
pub struct StorageRunSlot<T> {
	lifetime: Dependent<DebugLifetime>,
	value: T,
}

impl<T> StorageRunSlot<T> {
	pub fn lifetime(&self) -> DebugLifetime {
		self.lifetime.get()
	}

	pub fn value(&self) -> &T {
		&self.value
	}

	pub fn value_mut(&mut self) -> &mut T {
		&mut self.value
	}
}

// === Celled Storage === //

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
		self.inner.get(entity).map(|v| &**v)
	}

	pub fn try_get_mut(&mut self, entity: Entity) -> Option<&mut T> {
		self.inner.get_mut(entity).map(|v| &mut **v)
	}

	pub fn get(&self, entity: Entity) -> &T {
		self.try_get(entity)
			.unwrap_or_else(|| failed_to_find_component::<T>(entity))
	}

	pub fn get_mut(&mut self, entity: Entity) -> &mut T {
		self.try_get_mut(entity)
			.unwrap_or_else(|| failed_to_find_component::<T>(entity))
	}

	pub fn clear(&mut self) {
		self.inner.clear();
	}

	pub fn as_celled_view(&mut self) -> &mut CelledStorageView<T> {
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
		self.inner.get(entity)
	}

	pub fn try_get_cell_mut(&mut self, entity: Entity) -> Option<&mut RefCell<T>> {
		self.inner.get_mut(entity)
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
