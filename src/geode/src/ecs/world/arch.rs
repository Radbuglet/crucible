use crate::ecs::world::entities::EntityManager;
use crate::ecs::world::ids::{ArchGen, DirtyId, StorageId, StorageIdGenerator};
use crate::ecs::world::Entity;
use crate::util::free_list::FreeList;
use crate::util::iter_ext::{hash_iter, is_sorted, ExcludeSortedIter, MergeSortedIter};
use crate::util::number::{NumberGenMut, NumberGenRef, OptionalUsize};
use hashbrown::raw::RawTable;
use std::collections::hash_map::RandomState;
use std::fmt::{Debug, Formatter};
use thiserror::Error;

pub struct ArchManager {
	/// A monotonically increasing storage ID generator.
	storage_id_gen: StorageIdGenerator,

	/// A monotonically increasing archetype ID generator.
	arch_gen_gen: ArchGen,

	/// Maps component list hashes to archetypes. The first element of each entry is the hash and the
	/// second is the archetype index. Equality is checked against the corresponding
	/// `ArchetypeData.storages` buffer in `archetypes`.
	full_archetype_map: RawTable<(u64, usize)>,

	/// A free list of archetypes.
	archetypes: FreeList<WorldArchetype>,

	/// A global generator of dirty version IDs
	dirty_id_gen: DirtyId,

	/// An archetype hasher.
	hasher: RandomState,
}

impl Debug for ArchManager {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("ArchManager")
			.field("storage_id_gen", &self.storage_id_gen)
			.field("arch_gen_gen", &self.arch_gen_gen)
			.field("archetypes", &self.archetypes)
			.field("hasher", &self.hasher)
			.finish_non_exhaustive()
	}
}

impl Default for ArchManager {
	fn default() -> Self {
		Self {
			storage_id_gen: StorageIdGenerator::default(),
			arch_gen_gen: ArchGen::new(1).unwrap(),
			full_archetype_map: RawTable::new(),
			archetypes: FreeList::new(),
			dirty_id_gen: DirtyId::new(1).unwrap(),
			hasher: RandomState::new(),
		}
	}
}

impl ArchManager {
	pub fn new_storage_sync(&mut self) -> StorageId {
		self.storage_id_gen.try_generate_mut().unwrap()
	}

	pub fn new_storage_async(&self) -> StorageId {
		self.storage_id_gen.try_generate_ref().unwrap()
	}

	pub fn slot_to_handle(&self, index: usize) -> ArchHandle {
		ArchHandle {
			index,
			gen: self.archetypes[index].gen,
		}
	}

	pub fn move_to_arch(
		&mut self,
		entities: &mut EntityManager,
		entity_index: usize,
		arch_index: usize,
	) {
		let (gen, slot) = entities.locate_entity_raw_mut(entity_index);
		if let Some(index) = slot.arch_index.as_option() {
			self.archetypes[index].remove_entity(slot.index_in_arch, &mut self.dirty_id_gen);
		}

		let arch = &mut self.archetypes[arch_index];
		*slot = RawEntityArchLocator {
			arch_index: OptionalUsize::some(arch_index),
			index_in_arch: arch.entities().len(),
		};
		arch.push_entity(
			Entity {
				index: entity_index,
				gen,
			},
			&mut self.dirty_id_gen,
		);
	}

	pub fn get_arch(&self, handle: ArchHandle) -> Result<&WorldArchetype, ArchetypeDeadError> {
		self.archetypes
			.get(handle.index)
			.filter(|arch| arch.gen == handle.gen)
			.ok_or(ArchetypeDeadError(handle))
	}

	pub fn find_arch<I>(&self, comp_list_sorted: I) -> Option<ArchHandle>
	where
		I: IntoIterator<Item = StorageId>,
		I::IntoIter: ExactSizeIterator + Clone,
	{
		let comp_list_sorted = comp_list_sorted.into_iter();
		debug_assert!(is_sorted(comp_list_sorted.clone()));

		let hash = hash_iter(&self.hasher, comp_list_sorted.clone());
		let len = comp_list_sorted.len();
		self.find_arch_raw(hash, comp_list_sorted, len)
			.map(|index| ArchHandle {
				index,
				gen: self.archetypes[index].gen,
			})
	}

	fn find_arch_raw<I>(
		&self,
		comp_list_hash: u64,
		comp_list: I,
		comp_list_len: usize,
	) -> Option<usize>
	where
		I: Iterator<Item = StorageId> + Clone,
	{
		let existing =
			self.full_archetype_map
				.get(comp_list_hash, |(candidate_hash, candidate_index)| {
					if comp_list_hash != *candidate_hash {
						return false;
					}

					let candidate = &self.archetypes[*candidate_index];
					if candidate.storages.len() != comp_list_len {
						return false;
					}

					comp_list.clone().eq(candidate.storages.iter().copied())
				});

		existing.map(|(_, index)| *index)
	}

	// High effort code-dedup.
	fn arch_dest_or_insert_slow_base<G: for<'a> CompListGenerator<'a>>(
		&mut self,
		source_arch: OptionalUsize,
		comps_from_arch: G,
	) -> usize {
		// Find source archetype info
		let original_storages = match source_arch.as_option() {
			Some(source_arch) => &*self.archetypes[source_arch].storages,
			None => &[],
		};

		// Generate component list
		let (comp_list_len, comp_list) = comps_from_arch.make_iter(&original_storages);
		let comp_list_hash = hash_iter(&mut self.hasher, comp_list.clone());

		// Fetch archetype or register it.
		if let Some(index) = self.find_arch_raw(comp_list_hash, comp_list.clone(), comp_list_len) {
			index
		} else {
			let comp_list = comp_list.collect();
			let index = self.archetypes.add(WorldArchetype::new(
				self.arch_gen_gen.try_generate_mut().unwrap(),
				comp_list,
			));
			self.full_archetype_map
				.insert(comp_list_hash, (comp_list_hash, index), |(hash, _)| *hash);
			index
		}
	}

	pub fn arch_dest_for_addition(
		&mut self,
		source_arch: OptionalUsize,
		adding: StorageId,
	) -> usize {
		// TODO: Memoize conversions
		use std::iter::{once, Copied, Once};

		struct AddGen(StorageId);

		impl<'a> CompListGenerator<'a> for AddGen {
			type Iter = MergeSortedIter<Copied<std::slice::Iter<'a, StorageId>>, Once<StorageId>>;

			fn make_iter(&self, original_storages: &'a [StorageId]) -> (usize, Self::Iter) {
				debug_assert!(!original_storages.contains(&self.0));
				let iter = MergeSortedIter::new(original_storages.iter().copied(), once(self.0));
				let len = original_storages.len() + 1;
				(len, iter)
			}
		}

		self.arch_dest_or_insert_slow_base(source_arch, AddGen(adding))
	}

	pub fn arch_dest_for_deletion(
		&mut self,
		source_arch: OptionalUsize,
		removing: StorageId,
	) -> usize {
		use std::iter::{once, Copied, Once};

		struct DelGen(StorageId);

		impl<'a> CompListGenerator<'a> for DelGen {
			type Iter = ExcludeSortedIter<Copied<std::slice::Iter<'a, StorageId>>, Once<StorageId>>;

			fn make_iter(&self, original_storages: &'a [StorageId]) -> (usize, Self::Iter) {
				debug_assert!(original_storages.contains(&self.0));
				let iter = ExcludeSortedIter::new(original_storages.iter().copied(), once(self.0));
				let len = original_storages.len() - 1;
				(len, iter)
			}
		}

		self.arch_dest_or_insert_slow_base(source_arch, DelGen(removing))
	}
}

trait CompListGenerator<'a> {
	type Iter: Iterator<Item = StorageId> + Clone;

	fn make_iter(&self, comps: &'a [StorageId]) -> (usize, Self::Iter);
}

#[derive(Debug, Clone, Default)]
pub struct RawEntityArchLocator {
	/// The slot containing this entity.
	pub arch_index: OptionalUsize,

	/// The index of this entity within the archetype.
	pub index_in_arch: usize,
}

#[derive(Debug, Clone)]
pub struct EntityArchLocator {
	/// The archetype containing this entity.
	pub arch: ArchHandle,

	/// The index of this entity within the archetype.
	pub index_in_arch: usize,
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct ArchHandle {
	index: usize,
	gen: ArchGen,
}

#[derive(Debug, Clone)]
pub struct WorldArchetype {
	/// The archetype's current generation to distinguish it between other archetypes in the same
	/// slot. Archetypes are cleaned up once their entity count drops to zero.
	gen: ArchGen,

	/// The storages contained within this archetype.
	storages: Box<[StorageId]>,

	/// Entity IDs in this archetype.
	entity_ids: Vec<Entity>,

	/// ...and their corresponding dirty-tracking metadata nodes.
	entity_dirty_meta: Vec<DirtyEntityNode>,

	/// The head of the dirty nodes linked list.
	dirty_head: OptionalUsize,
}

#[derive(Debug, Clone)]
struct DirtyEntityNode {
	dirty_id: DirtyId,

	/// Previous dirty element in the list.
	prev_dirty: OptionalUsize,

	/// Next dirty element in the list.
	next_dirty: OptionalUsize,
}

impl Default for DirtyEntityNode {
	fn default() -> Self {
		Self {
			dirty_id: DirtyId::new(1).unwrap(),
			prev_dirty: OptionalUsize::NONE,
			next_dirty: OptionalUsize::NONE,
		}
	}
}

impl WorldArchetype {
	pub fn new(gen: ArchGen, storages: Box<[StorageId]>) -> Self {
		Self {
			gen,
			storages,
			entity_ids: Vec::new(),
			entity_dirty_meta: Vec::new(),
			dirty_head: OptionalUsize::NONE,
		}
	}

	fn relink_remove_dirty_inner(&mut self, index: usize) {
		// Old link layout:
		// [...] [old_node.prev | head] [old_node] [old_node.next] [...]
		//
		// New link layout:
		// [...] [old_node.prev | head] <-> [old_node.next] [...]

		let node = self.entity_dirty_meta[index].clone();

		if let Some(prev) = node.prev_dirty.as_option() {
			self.entity_dirty_meta[prev].next_dirty = node.next_dirty;
		} else {
			self.dirty_head = node.next_dirty;
		}

		if let Some(next) = node.next_dirty.as_option() {
			self.entity_dirty_meta[next].prev_dirty = node.prev_dirty;
		}
	}

	fn link_front_push_inner(&mut self, index: usize, dirty_id_gen: &mut DirtyId) {
		// Old link layout:
		// [head_ptr] [self.dirty_head] [...]
		//
		// New link layout:
		// [head_ptr] [index] [self.dirty_head] [...]

		let dirty_id = dirty_id_gen.try_generate_mut().unwrap();

		self.entity_dirty_meta[index] = DirtyEntityNode {
			dirty_id,
			prev_dirty: OptionalUsize::NONE,
			next_dirty: self.dirty_head,
		};

		if let Some(prev_head) = self.dirty_head.as_option() {
			self.entity_dirty_meta[prev_head].prev_dirty = OptionalUsize::some(index);
		}

		self.dirty_head = OptionalUsize::some(index);
	}

	fn push_entity(&mut self, entity: Entity, dirty_id_gen: &mut DirtyId) {
		self.entity_ids.push(entity);

		// This is just some temporary state for `link_front_push_inner` to initialize.
		self.entity_dirty_meta.push(DirtyEntityNode::default());
		self.link_front_push_inner(self.entity_dirty_meta.len() - 1, dirty_id_gen);
	}

	fn remove_entity(&mut self, index: usize, dirty_id_gen: &mut DirtyId) {
		let last_index = self.entity_dirty_meta.len() - 1;

		// Unlink last index from its surroundings
		self.relink_remove_dirty_inner(last_index);

		if index == last_index {
			// We're done here.
			return;
		}

		// Unlink `index` from its surroundings
		self.relink_remove_dirty_inner(index);

		// Perform swap removes
		self.entity_ids.swap_remove(index);
		self.entity_dirty_meta.swap_remove(index);

		// Push `index` to the front of the dirty list
		self.link_front_push_inner(index, dirty_id_gen);
	}

	pub fn entities(&self) -> &[Entity] {
		&self.entity_ids
	}

	pub fn dirty_version(&self) -> DirtyId {
		self.dirty_head
			.as_option()
			.map_or(DirtyId::new(1).unwrap(), |index| {
				self.entity_dirty_meta[index].dirty_id
			})
	}

	pub fn iter_dirties(&self, last_checked: DirtyId) -> WorldArchDirtyIter {
		WorldArchDirtyIter {
			arch: self,
			last_checked,
			iter_index: self.dirty_head.as_option(),
		}
	}
}

#[derive(Debug, Clone)]
pub struct WorldArchDirtyIter<'a> {
	arch: &'a WorldArchetype,
	last_checked: DirtyId,
	iter_index: Option<usize>,
}

impl<'a> Iterator for WorldArchDirtyIter<'a> {
	type Item = (DirtyId, usize);

	fn next(&mut self) -> Option<Self::Item> {
		let index = self.iter_index?;
		let node = &self.arch.entity_dirty_meta[index];

		if node.dirty_id >= self.last_checked {
			self.iter_index = node.next_dirty.as_option();
			Some((node.dirty_id, index))
		} else {
			None
		}
	}
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Error)]
#[error("archetype {0:?} is dead")]
pub struct ArchetypeDeadError(ArchHandle);
