use crate::util::{
	free_list::{AtomicFreeList, FreeList, SlotHandle, SlotState},
	number::{NonZeroU64Generator, NumberGenMut, OptionalUsize, U64Generator},
};
use std::num::NonZeroU64;

// === Newtypes === //

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct Entity(pub SlotHandle);

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct ArchetypeHandle {
	pub(super) index: u32,
	pub(super) gen: NonZeroU64,
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
struct StorageHandle(pub SlotHandle);

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct ArchSnapshotId(pub(super) NonZeroU64);

// === World === //

#[derive(Debug, Default)]
pub struct World {
	archetypes: FreeList<Archetype>,
	arch_gen_generator: U64Generator,
	entities: AtomicFreeList<EntitySlot>,
	storages: AtomicFreeList<StorageSlot>,
}

impl World {
	pub fn new() -> Self {
		Default::default()
	}

	pub fn spawn(&mut self) -> Entity {
		todo!()
	}

	pub fn despawn(&mut self, target: Entity) {
		todo!()
	}

	pub fn is_alive_now(&self, target: Entity) -> bool {
		todo!()
	}

	pub fn is_future(&self, target: Entity) -> bool {
		todo!()
	}

	pub fn is_not_condemned(&self, target: Entity) -> bool {
		todo!()
	}

	pub fn is_condemned(&self, target: Entity) -> bool {
		todo!()
	}

	pub fn get_entity_state(&self, target: Entity) -> EntityState {
		todo!()
	}

	pub fn new_storage(&mut self) {}
}

#[derive(Debug)]
struct EntitySlot {
	loc: Option<(ArchetypeHandle, usize)>,
}

#[derive(Debug)]
struct StorageSlot {
	contained_in: Vec<ArchetypeHandle>,
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum EntityState {
	Alive,
	Future,
	Condemned,
}

impl From<SlotState> for EntityState {
	fn from(state: SlotState) -> Self {
		match state {
			SlotState::Alive => Self::Alive,
			SlotState::Future => Self::Future,
			SlotState::Condemned => Self::Condemned,
		}
	}
}

// === Archetype === //

#[derive(Debug)]
pub struct Archetype {
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

impl Archetype {
	pub fn entities(&self) -> &[Entity] {
		self.entities.as_slice()
	}

	pub fn recent_changes(&self) -> ArchetypeChangesIter<'_> {
		ArchetypeChangesIter {
			target: self,
			this: self.links_head,
		}
	}

	fn push(&mut self, entity: Entity) -> usize {
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

	fn swap_remove(&mut self, index: usize) -> SwapRemoveResult {
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

#[derive(Debug, Clone)]
pub struct ArchetypeChangesIter<'a> {
	target: &'a Archetype,
	this: OptionalUsize,
}

impl Iterator for ArchetypeChangesIter<'_> {
	type Item = (usize, ArchSnapshotId);

	fn next(&mut self) -> Option<Self::Item> {
		let this_idx = self.this.as_option()?;
		let this_slot = &self.target.links[this_idx];
		self.this = this_slot.next;
		Some((this_idx, this_slot.version))
	}
}
