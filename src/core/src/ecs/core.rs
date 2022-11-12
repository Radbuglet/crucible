use std::{any::type_name, collections::HashMap, num::NonZeroU32, ops, sync::Mutex};

use derive_where::derive_where;

use crate::{
	debug::{error::ResultExt, lifetime::DebugLifetime},
	lang::polyfill::VecPoly,
	mem::free_list::PureFreeList,
};

// === Archetype === //

static FREE_ARCH_IDS: Mutex<PureFreeList<()>> = Mutex::new(PureFreeList::const_new());

#[derive(Debug)]
pub struct Archetype {
	lifetime: DebugLifetime,
	id: NonZeroU32,
	slots: PureFreeList<()>, // TODO: Improve slot packing with a hibitset.
}

impl Archetype {
	pub fn new() -> Self {
		// Generate archetype ID
		let mut free_arch_ids = FREE_ARCH_IDS.lock().unwrap_pretty();
		let (_, id) = free_arch_ids.add(());
		let id = id.checked_add(1).expect("created too many Archetypes.");
		let id = NonZeroU32::new(id).unwrap();

		// Construct archetype
		Self {
			lifetime: DebugLifetime::new(),
			id,
			slots: PureFreeList::new(),
		}
	}

	pub fn spawn(&mut self) -> Entity {
		let (_, slot) = self.slots.add(());

		Entity {
			lifetime: EntityLifetime {
				arch: self.lifetime,
				slot: DebugLifetime::new(),
			},
			arch_id: self.id,
			slot,
		}
	}

	pub fn despawn(&mut self, entity: Entity) {
		debug_assert_eq!(entity.arch_id, self.id);
		assert!(
			entity.lifetime.slot.is_possibly_alive(),
			"Attempted to despawn a dead entity."
		);

		entity.lifetime.slot.destroy();
		self.slots.remove(entity.slot);
	}
}

impl Drop for Archetype {
	fn drop(&mut self) {
		self.lifetime.destroy();

		let mut free_arch_ids = FREE_ARCH_IDS.lock().unwrap_pretty();
		free_arch_ids.remove(self.id.get() - 1);
	}
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
struct EntityLifetime {
	// TODO: Remove arch lifetime.
	arch: DebugLifetime,
	slot: DebugLifetime,
}

impl EntityLifetime {
	fn is_possibly_alive(self) -> bool {
		self.arch.is_possibly_alive() && self.slot.is_possibly_alive()
	}
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct Entity {
	lifetime: EntityLifetime,
	arch_id: NonZeroU32,
	slot: u32,
}

impl Entity {
	fn slot(&self) -> usize {
		self.slot as usize
	}
}

// === Storage === //

#[derive(Debug, Clone)]
#[derive_where(Default)]
#[repr(transparent)]
pub struct Storage<T> {
	// TODO: Replace with PerfectHashMap
	archetypes: HashMap<NonZeroU32, Vec<Option<(EntityLifetime, T)>>>,
}

impl<T> Storage<T> {
	pub fn new() -> Self {
		Self {
			archetypes: HashMap::new(),
		}
	}

	pub fn add(&mut self, entity: Entity, value: T) -> (Option<T>, &mut T) {
		assert!(
			entity.lifetime.is_possibly_alive(),
			"Attempted to attach a component to a dead entity."
		);

		let components = self
			.archetypes
			.entry(entity.arch_id)
			.or_insert_with(Vec::new);

		let slot = components.ensure_slot_with(entity.slot(), || None);
		let replaced = slot
			.replace((entity.lifetime, value))
			.map(|(lt, replaced)| {
				assert!(
					lt.is_possibly_alive(),
					"Replaced dangling component belonging to a destroyed entity. Please detach all \
					 components belonging to an entity before freeing them."
				);
				replaced
			});

		(replaced, &mut slot.as_mut().unwrap().1)
	}

	pub fn remove(&mut self, entity: Entity) -> Option<T> {
		assert!(
			entity.lifetime.is_possibly_alive(),
			"Attempted to remove a component to a dead entity. Please detach components belonging to an entity *before* freeing them."
		);

		let archetype = self.archetypes.get_mut(&entity.arch_id)?;
		let removed = archetype[entity.slot()].take().map(|(_, value)| value);

		while archetype.last().is_none() {
			archetype.pop();
		}

		removed
	}

	pub fn remove_many<I>(&mut self, entities: I)
	where
		I: IntoIterator<Item = Entity>,
	{
		for entity in entities {
			self.remove(entity);
		}
	}

	pub fn try_get(&self, entity: Entity) -> Option<&T> {
		assert!(entity.lifetime.is_possibly_alive());

		self.archetypes
			.get(&entity.arch_id)?
			.get(entity.slot())?
			.as_ref()
			.map(|(_, value)| value)
	}

	pub fn try_get_mut(&mut self, entity: Entity) -> Option<&mut T> {
		assert!(entity.lifetime.is_possibly_alive());

		self.archetypes
			.get_mut(&entity.arch_id)?
			.get_mut(entity.slot())?
			.as_mut()
			.map(|(_, value)| value)
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
		self.archetypes.clear();
	}
}

impl<T> ops::Index<Entity> for Storage<T> {
	type Output = T;

	fn index(&self, index: Entity) -> &Self::Output {
		self.get(index)
	}
}

impl<T> ops::IndexMut<Entity> for Storage<T> {
	fn index_mut(&mut self, index: Entity) -> &mut Self::Output {
		self.get_mut(index)
	}
}

pub(super) fn failed_to_find_component<T>(entity: Entity) -> ! {
	panic!(
		"failed to find entity {entity:?} with component {}",
		type_name::<T>()
	);
}