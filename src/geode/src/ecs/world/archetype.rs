use std::num::NonZeroU64;

use derive_where::derive_where;

use crate::util::free_list::FreeList;
use crate::util::number::NonZeroU64Generator;
use crate::util::number::NumberGenMut;
use crate::util::number::OptionalUsize;

use super::ArchSnapshotId;
use super::ArchetypeHandle;
use super::Entity;

#[derive(Debug)]
#[derive_where(Default)]
pub struct ArchetypeTable<M> {
	// A free list of all archetypes in the storage.
	archetypes: FreeList<Archetype<M>>,

	// A generator for archetype generations.
	gen_generator: NonZeroU64Generator,
}

impl<M> ArchetypeTable<M> {
	pub fn create_archetype(&mut self, meta: M) -> ArchetypeHandle {
		let gen = self.gen_generator.generate_mut();
		let index = self.archetypes.add(Archetype {
			meta,
			gen,
			snapshot_generator: NonZeroU64Generator::default(),
			entities: Vec::new(),
			links: Vec::new(),
			links_head: OptionalUsize::NONE,
		});

		ArchetypeHandle { index, gen }
	}

	pub fn delete_archetype(&mut self, handle: ArchetypeHandle) -> M {
		assert_eq!(self.archetypes[handle.index].gen, handle.gen);
		let archetype = self.archetypes.free(handle.index).unwrap();
		archetype.meta
	}

	pub fn get_archetype(&self, handle: ArchetypeHandle) -> Option<&Archetype<M>> {
		self.archetypes
			.get(handle.index)
			.filter(|arch| arch.gen == handle.gen)
	}

	pub fn get_archetype_mut(&mut self, handle: ArchetypeHandle) -> Option<&mut Archetype<M>> {
		self.archetypes
			.get_mut(handle.index)
			.filter(|arch| arch.gen == handle.gen)
	}
}

#[derive(Debug)]
pub struct Archetype<M> {
	// User supplied metadata per archetype.
	meta: M,

	// The archetype's generation. Used to distinguish
	// between multiple archetypes in the same slot.
	gen: NonZeroU64,

	// A generator for change snapshot IDs.
	snapshot_generator: NonZeroU64Generator,

	// The entities in the archetype...
	entities: Vec<Entity>,

	// ...and their corresponding linked list of changes.
	links: Vec<ArchSlot>,

	// The head of the changes linked list.
	links_head: OptionalUsize,
}

impl<M> Archetype<M> {
	pub fn meta(&self) -> &M {
		&self.meta
	}

	pub fn meta_mut(&mut self) -> &mut M {
		&mut self.meta
	}

	pub fn entities(&self) -> &[Entity] {
		self.entities.as_slice()
	}

	pub fn push(&mut self, entity: Entity) -> usize {
		let new_index = self.entities.len();

		self.entities.push(entity);

		// Link new_index -> links_head
		self.links.push(ArchSlot {
			prev: OptionalUsize::NONE,
			next: self.links_head,
			version: ArchSnapshotId(self.snapshot_generator.generate_mut()),
		});

		// Link new_index <- links_head
		if let Some(links_head) = self.links_head.as_option() {
			self.links[links_head].prev = OptionalUsize::some(new_index);
		}

		// Link head -> new_index
		self.links_head = OptionalUsize::some(new_index);

		new_index
	}

	fn unlink(&mut self, index: usize) {
		let target = self.links[index].clone();

		// Update prev (or head) -> next
		if let Some(prev) = target.prev.as_option() {
			self.links[prev].next = target.next;
		} else {
			debug_assert_eq!(self.links_head.as_option(), Some(index));
			self.links_head = target.next;
		}

		// Update prev <- next
		if let Some(next) = target.next.as_option() {
			self.links[next].prev = target.prev;
		}
	}

	pub fn swap_remove(&mut self, index: usize) -> SwapRemoveResult {
		// Remove entity
		let removed = self.entities.swap_remove(index);
		let moved = self.entities.get(index).copied();

		// Unlink target node and swap remove it
		self.unlink(index);
		self.links.swap_remove(index);

		// Relink moved node
		if let Some(moved) = self.links.get(index).cloned() {
			// Relink prev -> index
			if let Some(prev) = moved.prev.as_option() {
				self.links[prev].next = OptionalUsize::some(index);
			} else {
				self.links_head = OptionalUsize::some(index);
			}

			// Relink index <- next
			if let Some(next) = moved.next.as_option() {
				self.links[next].prev = OptionalUsize::some(index);
			}
		}

		SwapRemoveResult { removed, moved }
	}

	pub fn recent_changes(&self) -> ArchetypeChangesIter<'_, M> {
		ArchetypeChangesIter {
			target: self,
			this: self.links_head,
		}
	}
}

#[derive(Debug, Clone)]
pub struct SwapRemoveResult {
	pub removed: Entity,
	pub moved: Option<Entity>,
}

#[derive(Debug, Clone)]
struct ArchSlot {
	prev: OptionalUsize,
	next: OptionalUsize,
	version: ArchSnapshotId,
}

#[derive(Debug)]
#[derive_where(Clone)]
pub struct ArchetypeChangesIter<'a, M> {
	target: &'a Archetype<M>,
	this: OptionalUsize,
}

impl<M> Iterator for ArchetypeChangesIter<'_, M> {
	type Item = (usize, ArchSnapshotId);

	fn next(&mut self) -> Option<Self::Item> {
		let this_idx = self.this.as_option()?;
		let this_slot = &self.target.links[this_idx];
		self.this = this_slot.next;
		Some((this_idx, this_slot.version))
	}
}
