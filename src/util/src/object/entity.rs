use derive_where::derive_where;
use fnv::FnvHashMap;
use std::{
	any::{type_name, Any, TypeId},
	cell::{Ref, RefCell, RefMut},
	collections::hash_map,
	iter::repeat_with,
	num::NonZeroU64,
	sync::atomic::{AtomicU64, Ordering},
};

// === Database === //

pub fn storage<T: 'static>() -> &'static Storage<T> {
	thread_local! {
		static DATABASE: RefCell<FnvHashMap<TypeId, &'static dyn Any>> = Default::default();
	}

	DATABASE.with(|db| {
		db.borrow_mut()
			.entry(TypeId::of::<T>())
			.or_insert_with(|| &*Box::leak(Box::new(Storage::<T>::default())))
			.downcast_ref::<Storage<T>>()
			.unwrap()
	})
}

// === Storage === //

const BLOCK_SIZE: usize = 128;

type StorageSlot<T> = RefCell<Option<T>>;

#[derive(Debug)]
#[derive_where(Default)]
pub struct Storage<T: 'static>(RefCell<StorageInner<T>>);

#[derive(Debug)]
#[derive_where(Default)]
struct StorageInner<T: 'static> {
	free_slots: Vec<&'static StorageSlot<T>>,
	mappings: FnvHashMap<Entity, &'static StorageSlot<T>>,
}

impl<T: 'static> Storage<T> {
	pub fn insert(&self, entity: Entity, value: T) -> Option<T> {
		let me = &mut *self.0.borrow_mut();

		let slot = match me.mappings.entry(entity) {
			hash_map::Entry::Occupied(entry) => entry.get(),
			hash_map::Entry::Vacant(entry) => {
				if me.free_slots.is_empty() {
					let block = repeat_with(StorageSlot::default)
						.take(BLOCK_SIZE)
						.collect::<Vec<_>>()
						.leak();

					me.free_slots.extend(block.into_iter().map(|v| &*v));
				}

				let slot = me.free_slots.pop().unwrap();
				entry.insert(slot);
				slot
			}
		};

		slot.borrow_mut().replace(value)
	}

	pub fn remove(&self, entity: Entity) -> Option<T> {
		let mut me = self.0.borrow_mut();

		if let Some(slot) = me.mappings.remove(&entity) {
			me.free_slots.push(slot);
			slot.borrow_mut().take()
		} else {
			None
		}
	}

	fn try_get_slot(&self, entity: Entity) -> Option<&'static StorageSlot<T>> {
		self.0.borrow().mappings.get(&entity).copied()
	}

	fn get_slot(&self, entity: Entity) -> &'static StorageSlot<T> {
		self.try_get_slot(entity).unwrap_or_else(|| {
			panic!(
				"Failed to find component of type {} for {:?}.",
				type_name::<T>(),
				entity,
			);
		})
	}

	pub fn try_get(&self, entity: Entity) -> Option<Ref<'static, T>> {
		self.try_get_slot(entity)
			.map(|slot| Ref::map(slot.borrow(), |v| v.as_ref().unwrap()))
	}

	pub fn try_get_mut(&self, entity: Entity) -> Option<RefMut<'static, T>> {
		self.try_get_slot(entity)
			.map(|slot| RefMut::map(slot.borrow_mut(), |v| v.as_mut().unwrap()))
	}

	pub fn get(&self, entity: Entity) -> Ref<'static, T> {
		Ref::map(self.get_slot(entity).borrow(), |v| v.as_ref().unwrap())
	}

	pub fn get_mut(&self, entity: Entity) -> RefMut<'static, T> {
		RefMut::map(self.get_slot(entity).borrow_mut(), |v| v.as_mut().unwrap())
	}

	pub fn has(&self, entity: Entity) -> bool {
		self.try_get_slot(entity).is_some()
	}
}

// === Entity === //

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct Entity(NonZeroU64);

impl Entity {
	pub fn new() -> Self {
		static ID_GEN: AtomicU64 = AtomicU64::new(1);

		Self(NonZeroU64::new(ID_GEN.fetch_add(1, Ordering::Relaxed)).unwrap())
	}

	pub fn with<T: 'static>(self, comp: T) -> Self {
		self.insert(comp);
		self
	}

	pub fn insert<T: 'static>(self, comp: T) -> Option<T> {
		storage::<T>().insert(self, comp)
	}

	pub fn remove<T: 'static>(self) -> Option<T> {
		storage::<T>().remove(self)
	}

	pub fn try_get<T: 'static>(self) -> Option<Ref<'static, T>> {
		storage::<T>().try_get(self)
	}

	pub fn try_get_mut<T: 'static>(self) -> Option<RefMut<'static, T>> {
		storage::<T>().try_get_mut(self)
	}

	pub fn get<T: 'static>(self) -> Ref<'static, T> {
		storage::<T>().get(self)
	}

	pub fn get_mut<T: 'static>(self) -> RefMut<'static, T> {
		storage::<T>().get_mut(self)
	}

	pub fn has<T: 'static>(self) -> bool {
		storage::<T>().has(self)
	}
}
