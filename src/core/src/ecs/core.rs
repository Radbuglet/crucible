use std::{
	any::type_name,
	collections::hash_map,
	collections::HashMap,
	num::NonZeroU32,
	ops::{self, Deref, DerefMut},
	sync::Mutex,
};

use derive_where::derive_where;

use crate::{
	debug::{
		error::ResultExt,
		lifetime::{DebugLifetime, LifetimeDependent, LifetimeOwner},
	},
	lang::polyfill::VecPoly,
	mem::{free_list::PureFreeList, ptr::PointeeCastExt},
};

use super::query::{Query, QueryIter};

// === Handles === //

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct ArchetypeId {
	pub lifetime: DebugLifetime,
	pub id: NonZeroU32,
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
	archetypes: HashMap<NonZeroU32, StorageRun<T>>,
}

impl<T> Storage<T> {
	pub fn new() -> Self {
		Self {
			archetypes: HashMap::new(),
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

	pub fn get_run_view(&self, archetype: ArchetypeId) -> &StorageRunView<T> {
		match self.get_run(archetype) {
			Some(run) => run.as_view(),
			None => StorageRunView::new_empty(),
		}
	}

	pub fn get_run_view_mut(&mut self, archetype: ArchetypeId) -> &mut StorageRunView<T> {
		match self.get_run_mut(archetype) {
			Some(run) => run.as_mut_view(),
			None => StorageRunView::new_empty(),
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
			.map(|(_, v)| v)
	}

	pub fn get_mut(&mut self, entity: Entity) -> Option<&mut T> {
		assert!(entity.lifetime.is_possibly_alive());

		self.archetypes
			.get_mut(&entity.arch.id)?
			.get_mut(entity.slot)
			.map(|(_, v)| v)
	}

	pub fn clear(&mut self) {
		self.archetypes.clear();
	}

	pub fn query_in_ref(&self, archetype: ArchetypeId) -> QueryIter<(&StorageRunView<T>,)> {
		(self,).query_in(archetype)
	}

	pub fn query_in_mut(&mut self, archetype: ArchetypeId) -> QueryIter<(&mut StorageRunView<T>,)> {
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

type StorageRunSlot<T> = Option<(LifetimeDependent<DebugLifetime>, T)>;

#[derive(Debug, Clone)]
#[derive_where(Default)]
pub struct StorageRun<T> {
	comps: Vec<StorageRunSlot<T>>,
}

impl<T> StorageRun<T> {
	pub fn new() -> Self {
		Self { comps: Vec::new() }
	}

	pub fn add(&mut self, entity: Entity, value: T) -> (Option<T>, &mut T) {
		let slot = self.comps.ensure_slot_with(entity.slot_usize(), || None);
		let replaced = slot
			.replace((LifetimeDependent::new(entity.lifetime), value))
			.map(|(_, replaced)| replaced);

		(replaced, &mut slot.as_mut().unwrap().1)
	}

	pub fn remove(&mut self, slot: u32) -> Option<T> {
		let removed = self.comps[slot as usize].take().map(|(_, value)| value);

		while matches!(self.comps.last(), Some(None)) {
			self.comps.pop();
		}

		removed
	}

	pub fn as_view(&self) -> &StorageRunView<T> {
		unsafe {
			self.comps
				.as_slice()
				.cast_ref_via_ptr(|slice: *const [StorageRunSlot<T>]| {
					slice as *const StorageRunView<T>
				})
		}
	}

	pub fn as_mut_view(&mut self) -> &mut StorageRunView<T> {
		unsafe {
			self.comps
				.as_mut_slice()
				.cast_mut_via_ptr(|slice: *mut [StorageRunSlot<T>]| slice as *mut StorageRunView<T>)
		}
	}
}

impl<T> Deref for StorageRun<T> {
	type Target = StorageRunView<T>;

	fn deref(&self) -> &Self::Target {
		self.as_view()
	}
}

impl<T> DerefMut for StorageRun<T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		self.as_mut_view()
	}
}

#[repr(transparent)]
pub struct StorageRunView<T>([StorageRunSlot<T>]);

impl<T> StorageRunView<T> {
	pub fn new_empty<'a>() -> &'a mut StorageRunView<T> {
		let slice: &'a mut [StorageRunSlot<T>] = &mut [];

		unsafe { slice.cast_mut_via_ptr(|slice| slice as *mut StorageRunView<T>) }
	}

	pub fn get(&self, slot: u32) -> Option<(DebugLifetime, &T)> {
		match self.0.get(slot as usize) {
			Some(Some((lt, value))) => Some((lt.lifetime(), value)),
			_ => None,
		}
	}

	pub fn get_mut(&mut self, slot: u32) -> Option<(DebugLifetime, &mut T)> {
		match self.0.get_mut(slot as usize) {
			Some(Some((lt, value))) => Some((lt.lifetime(), value)),
			_ => None,
		}
	}

	pub fn max_slot(&self) -> u32 {
		self.0.len() as u32
	}
}

// === ArchetypeMap === //

#[derive(Debug, Clone)]
#[derive_where(Default)]
pub struct ArchetypeMap<T> {
	map: HashMap<NonZeroU32, (LifetimeDependent<DebugLifetime>, T)>,
}

impl<T> ArchetypeMap<T> {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn add(&mut self, id: ArchetypeId, value: T) -> Option<T> {
		self.map
			.insert(id.id, (LifetimeDependent::new(id.lifetime), value))
			.map(|(_, v)| v)
	}

	pub fn remove(&mut self, id: ArchetypeId) -> Option<T> {
		self.map.remove(&id.id).map(|(_, v)| v)
	}

	pub fn remove_many<I>(&mut self, archetypes: I)
	where
		I: IntoIterator<Item = ArchetypeId>,
	{
		for arch in archetypes {
			self.remove(arch);
		}
	}

	pub fn try_get(&self, id: ArchetypeId) -> Option<&T> {
		self.map.get(&id.id).map(|(_, v)| v)
	}

	pub fn try_get_mut(&mut self, id: ArchetypeId) -> Option<&mut T> {
		self.map.get_mut(&id.id).map(|(_, v)| v)
	}

	pub fn get(&self, id: ArchetypeId) -> &T {
		self.try_get(id)
			.unwrap_or_else(|| failed_to_find_archetype_meta::<T>(id))
	}

	pub fn get_mut(&mut self, id: ArchetypeId) -> &mut T {
		self.try_get_mut(id)
			.unwrap_or_else(|| failed_to_find_archetype_meta::<T>(id))
	}

	pub fn clear(&mut self) {
		self.map.clear();
	}

	pub fn iter(&self) -> ArchetypeMapIter<T> {
		ArchetypeMapIter {
			iter: self.map.iter(),
		}
	}

	pub fn iter_mut(&mut self) -> ArchetypeMapIterMut<T> {
		ArchetypeMapIterMut {
			iter: self.map.iter_mut(),
		}
	}
}

impl<T> ops::Index<ArchetypeId> for ArchetypeMap<T> {
	type Output = T;

	fn index(&self, index: ArchetypeId) -> &Self::Output {
		self.get(index)
	}
}

impl<T> ops::IndexMut<ArchetypeId> for ArchetypeMap<T> {
	fn index_mut(&mut self, index: ArchetypeId) -> &mut Self::Output {
		self.get_mut(index)
	}
}

#[derive(Debug, Clone)]
pub struct ArchetypeMapIter<'a, T> {
	iter: hash_map::Iter<'a, NonZeroU32, (LifetimeDependent<DebugLifetime>, T)>,
}

impl<'a, T> Iterator for ArchetypeMapIter<'a, T> {
	type Item = (ArchetypeId, &'a T);

	fn next(&mut self) -> Option<Self::Item> {
		self.iter.next().map(|(&id, (lt, val))| {
			(
				ArchetypeId {
					id,
					lifetime: lt.lifetime(),
				},
				val,
			)
		})
	}
}

impl<'a, T> IntoIterator for &'a ArchetypeMap<T> {
	type Item = (ArchetypeId, &'a T);
	type IntoIter = ArchetypeMapIter<'a, T>;

	fn into_iter(self) -> Self::IntoIter {
		self.iter()
	}
}

#[derive(Debug)]
pub struct ArchetypeMapIterMut<'a, T> {
	iter: hash_map::IterMut<'a, NonZeroU32, (LifetimeDependent<DebugLifetime>, T)>,
}

impl<'a, T> Iterator for ArchetypeMapIterMut<'a, T> {
	type Item = (ArchetypeId, &'a mut T);

	fn next(&mut self) -> Option<Self::Item> {
		self.iter.next().map(|(&id, (lt, val))| {
			(
				ArchetypeId {
					id,
					lifetime: lt.lifetime(),
				},
				val,
			)
		})
	}
}

impl<'a, T> IntoIterator for &'a mut ArchetypeMap<T> {
	type Item = (ArchetypeId, &'a mut T);
	type IntoIter = ArchetypeMapIterMut<'a, T>;

	fn into_iter(self) -> Self::IntoIter {
		self.iter_mut()
	}
}

pub(super) fn failed_to_find_archetype_meta<T>(id: ArchetypeId) -> ! {
	panic!(
		"failed to find archetype {id:?} with metadata of type {}",
		type_name::<T>()
	);
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
