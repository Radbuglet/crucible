use std::{
	any::type_name,
	collections::{HashMap, HashSet},
	num::NonZeroU32,
	ops,
	sync::Mutex,
};

use derive_where::derive_where;

use crate::{
	debug::{
		error::ResultExt,
		lifetime::{DebugLifetime, Dependable, Dependent, LifetimeOwner},
	},
	lang::polyfill::VecPoly,
	mem::{
		auto_map::{AutoHashMap, CanForget},
		free_list::PureFreeList,
	},
};

use super::query::{Query, QueryIter};

// === Handles === //

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct ArchetypeId {
	pub lifetime: DebugLifetime,
	pub id: NonZeroU32,
}

impl Dependable for ArchetypeId {
	fn inc_dep(self) {
		self.lifetime.inc_dep();
	}

	fn dec_dep(self) {
		self.lifetime.dec_dep();
	}
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct Entity {
	pub lifetime: DebugLifetime,
	pub arch: ArchetypeId,
	pub slot: u32,
}

impl Entity {
	pub fn slot_usize(&self) -> usize {
		self.slot as usize
	}
}

impl Dependable for Entity {
	fn inc_dep(self) {
		self.lifetime.inc_dep();
	}

	fn dec_dep(self) {
		self.lifetime.dec_dep();
	}
}

// === Containers === //

pub type EntityMap<T> = HashMap<Dependent<Entity>, T>;
pub type EntitySet = HashSet<Dependent<Entity>>;
pub type ArchetypeMap<T> = HashMap<Dependent<ArchetypeId>, T>;
pub type ArchetypeSet = HashSet<Dependent<ArchetypeId>>;

// === Archetype === //

static ARCH_ID_FREE_LIST: Mutex<PureFreeList<()>> = Mutex::new(PureFreeList::const_new());

#[derive(Debug)]
pub struct Archetype {
	id: NonZeroU32,
	lifetime: LifetimeOwner<DebugLifetime>,
	slots: PureFreeList<LifetimeOwner<DebugLifetime>>,
}

impl Archetype {
	pub fn new() -> Self {
		// Generate archetype ID
		let mut free_arch_ids = ARCH_ID_FREE_LIST.lock().unwrap_pretty();
		let (_, id) = free_arch_ids.add(());
		let id = id.checked_add(1).expect("created too many archetypes.");
		let id = NonZeroU32::new(id).unwrap();

		// Construct archetype
		Self {
			id,
			lifetime: LifetimeOwner(DebugLifetime::new()),
			slots: PureFreeList::new(),
		}
	}

	pub fn spawn(&mut self) -> Entity {
		let (lifetime, slot) = self.slots.add(LifetimeOwner(DebugLifetime::new()));

		assert_ne!(slot, u32::MAX, "spawned too many entities");

		Entity {
			lifetime: lifetime.0,
			arch: self.id(),
			slot,
		}
	}

	pub fn despawn(&mut self, entity: Entity) {
		debug_assert_eq!(entity.arch.id, self.id);
		assert!(
			entity.lifetime.is_possibly_alive(),
			"Attempted to despawn a dead entity."
		);

		let _ = self.slots.remove(entity.slot);
	}

	pub fn id(&self) -> ArchetypeId {
		ArchetypeId {
			lifetime: self.lifetime.0,
			id: self.id,
		}
	}

	pub fn entities(&self) -> ArchetypeIter {
		ArchetypeIter {
			archetype: self,
			slot: 0,
		}
	}
}

impl Default for Archetype {
	fn default() -> Self {
		Self::new()
	}
}

impl Drop for Archetype {
	fn drop(&mut self) {
		let mut free_arch_ids = ARCH_ID_FREE_LIST.lock().unwrap_pretty();
		free_arch_ids.remove(self.id.get() - 1);
	}
}

#[derive(Debug, Clone)]
pub struct ArchetypeIter<'a> {
	archetype: &'a Archetype,
	slot: u32,
}

impl Iterator for ArchetypeIter<'_> {
	type Item = Entity;

	fn next(&mut self) -> Option<Self::Item> {
		let slots = self.archetype.slots.slots();

		loop {
			let slot = self.slot;
			self.slot += 1;
			match slots.get(slot as usize) {
				Some(Some((lt, _))) => {
					break Some(Entity {
						lifetime: lt.0,
						arch: self.archetype.id(),
						slot,
					})
				}
				Some(None) => {}
				None => break None,
			}
		}
	}
}

impl<'a> IntoIterator for &'a Archetype {
	type Item = Entity;
	type IntoIter = ArchetypeIter<'a>;

	fn into_iter(self) -> Self::IntoIter {
		self.entities()
	}
}

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

	pub fn get_run_mut(&mut self, archetype: ArchetypeId) -> Option<&mut StorageRun<T>> {
		assert!(archetype.lifetime.is_possibly_alive());

		self.archetypes.get_mut(&archetype.id)
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
			Some(run) => run.as_mut_slice(),
			None => &mut [],
		}
	}

	pub fn get_or_create_run(&mut self, archetype: ArchetypeId) -> &mut StorageRun<T> {
		assert!(archetype.lifetime.is_possibly_alive());

		self.archetypes
			.entry(archetype.id)
			.or_insert_with(Default::default)
	}

	pub fn add(&mut self, entity: Entity, value: T) -> (Option<T>, &mut T) {
		self.get_or_create_run(entity.arch).add(entity, value)
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

		self.archetypes
			.get_mut(&entity.arch.id)?
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

// === Tests === //

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	#[should_panic]
	fn uaf_detection_tripped() {
		let mut arch_player = Archetype::new();

		let player = arch_player.spawn();

		let mut storage = Storage::new();
		storage.add(player, ());

		arch_player.despawn(player);
	}

	#[test]
	fn uaf_detection_not_tripped() {
		let mut arch_player = Archetype::new();

		let player = arch_player.spawn();

		let mut storage = Storage::new();
		storage.add(player, ());

		storage.remove(player);
		arch_player.despawn(player);
	}
}
