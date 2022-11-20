use std::{
	any::type_name, collections::hash_map, collections::HashMap, num::NonZeroU32, ops, sync::Mutex,
};

use derive_where::derive_where;

use crate::{
	debug::{
		error::ResultExt,
		lifetime::{DebugLifetime, LifetimeDependent, LifetimeOwner},
	},
	lang::polyfill::VecPoly,
	mem::free_list::PureFreeList,
};

// === Archetype === //

static FREE_ARCH_IDS: Mutex<PureFreeList<()>> = Mutex::new(PureFreeList::const_new());

#[derive(Debug)]
pub struct Archetype {
	id: NonZeroU32,
	lifetime: LifetimeOwner<DebugLifetime>,
	slots: PureFreeList<LifetimeOwner<DebugLifetime>>,
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
			id,
			lifetime: LifetimeOwner(DebugLifetime::new()),
			slots: PureFreeList::new(),
		}
	}

	pub fn spawn(&mut self) -> Entity {
		let (lifetime, slot) = self.slots.add(LifetimeOwner(DebugLifetime::new()));

		Entity {
			own_lifetime: lifetime.0,
			arch_lifetime: self.lifetime.0,
			arch_id: self.id,
			slot,
		}
	}

	pub fn despawn(&mut self, entity: Entity) {
		debug_assert_eq!(entity.arch_id, self.id);
		assert!(
			entity.own_lifetime.is_possibly_alive(),
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
		let mut free_arch_ids = FREE_ARCH_IDS.lock().unwrap_pretty();
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
						arch_id: self.archetype.id,
						arch_lifetime: self.archetype.lifetime.0,
						own_lifetime: lt.0,
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

// === Handles === //

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct ArchetypeId {
	lifetime: DebugLifetime,
	id: NonZeroU32,
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct Entity {
	arch_lifetime: DebugLifetime, // TODO: Store in `own_lifetime`'s "associated metadata" once implemented.
	own_lifetime: DebugLifetime,
	arch_id: NonZeroU32,
	slot: u32,
}

impl Entity {
	pub fn arch(&self) -> ArchetypeId {
		ArchetypeId {
			lifetime: self.arch_lifetime,
			id: self.arch_id,
		}
	}

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
	archetypes: HashMap<NonZeroU32, Vec<Option<(LifetimeDependent<DebugLifetime>, T)>>>,
}

impl<T> Storage<T> {
	pub fn new() -> Self {
		Self {
			archetypes: HashMap::new(),
		}
	}

	pub fn add(&mut self, entity: Entity, value: T) -> (Option<T>, &mut T) {
		let components = self
			.archetypes
			.entry(entity.arch_id)
			.or_insert_with(Vec::new);

		let slot = components.ensure_slot_with(entity.slot(), || None);
		let replaced = slot
			.replace((LifetimeDependent::new(entity.own_lifetime), value))
			.map(|(_, replaced)| replaced);

		(replaced, &mut slot.as_mut().unwrap().1)
	}

	pub fn remove(&mut self, entity: Entity) -> Option<T> {
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
		assert!(entity.own_lifetime.is_possibly_alive());

		self.archetypes
			.get(&entity.arch_id)?
			.get(entity.slot())?
			.as_ref()
			.map(|(_, value)| value)
	}

	pub fn try_get_mut(&mut self, entity: Entity) -> Option<&mut T> {
		assert!(entity.own_lifetime.is_possibly_alive());

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

// === ArchetypeMap === //

#[derive(Debug, Clone)]
#[derive_where(Default)]
pub struct ArchetypeMap<T> {
	// TODO: Replace with PerfectHashMap
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
