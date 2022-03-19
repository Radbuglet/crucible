use crate::util::free_list::FreeList;
use crate::util::number::{usize_has_mask, usize_msb_mask, NumberGenExt, OptionalUsize};
use crossbeam::queue::SegQueue;
use hashbrown::raw::RawTable;
use std::collections::{HashMap, HashSet, VecDeque};
use std::marker::PhantomData;
use std::mem::replace;
use std::num::NonZeroU64;
use std::ops::Deref;
use std::sync::atomic::{AtomicIsize, AtomicU64, AtomicUsize, Ordering};

// === Identifier types === //

/// An entity generation; used to distinguish between multiple distinct entities in a single slot.
pub type EntityGen = NonZeroU64;
pub type AtomicEntityGen = AtomicU64;

/// The unique identifier of a storage.
pub type StorageId = NonZeroU64;
pub type AtomicStorageId = AtomicU64;

/// An archetype generation; used to distinguish between multiple distinct archetypes in a single slot.
pub type ArchGen = NonZeroU64;

/// An identifier for a snapshot in the archetype's history. Used to lazily bring storages up-to-date.
pub type DirtyId = u64;

// === World data structures === //

#[derive(Debug, Clone, Default)]
pub struct World {
	// === Entity management === //
	/// The last generation to be flushed.
	last_flushed_gen: EntityGen,

	/// A monotonically increasing generation counter.
	generation_gen: AtomicEntityGen,

	/// A buffer of entity slots; never shrinks. Used to check for liveness and to find containing
	/// archetypes for archetype moving.
	slots: Vec<EntitySlot>,

	/// A list of all the free slots; only updated every flush.
	free_slots: Vec<usize>,

	/// A queue of entity actions encoded as a bunch of `usize`s.
	///
	/// Each world handle commits a vector of its actions to this queue independently to reduce the
	/// amount of required synchronization.
	///
	/// ## Encoding scheme
	///
	/// The buffer starts out in the command context.
	///
	/// In the command context:
	///
	/// - The most significant bit indicates whether this is a deletion (true) or an archetypal
	///   adjustment (false).
	/// - The other bits indicate the entity index.
	/// - The presence of this instruction transitions the decoder to archetype listing context.
	///
	/// In the archetype listing context:
	///
	/// - The most significant bit indicates whether this is a continuation (true) or a terminator
	///   bringing us back to the command context (false).
	/// - The 2nd most significant bit indicates whether this is a deletion (true) or a deletion (false).
	///
	/// This encoding scheme allows us to pack deletion and archetype data in the same buffer and
	/// implement bundles in an efficient manner.
	mt_action_queue: SegQueue<Box<[usize]>>,

	// === Archetype management === //
	/// A monotonically increasing storage ID generator.
	storage_id_gen: AtomicStorageId,

	/// Maps component list hashes to archetypes. The first element of each entry is the hash and the
	/// second is the archetype index. Equality is checked against the corresponding
	/// `ArchetypeData.storages` buffer in `archetypes`.
	full_archetypes: RawTable<(u64, usize)>,

	/// A free list of archetypes.
	archetypes: FreeList<ArchetypeData>,

	/// Maps binary archetype conversions to their target archetype ID to avoid looking through the
	/// `full_archetypes` map—the assumption being that there is a finite number of routines promoting
	/// entities in a deterministic order.
	memoized_conversions: HashMap<Conversion, ArchId>,
}

#[derive(Debug, Clone)]
struct EntitySlot {
	/// The generation of the entity which is currently living in this slot or `None`.
	gen: Option<EntityGen>,

	/// The index of the containing archetype.
	arch: OptionalUsize,

	/// The index of the entity within that archetype.
	index_in_arch: usize,
}

#[derive(Error)]
#[error("entity {0} is dead")]
pub struct EntityDeadError(Entity);

#[derive(Debug, Clone)]
struct ArchetypeData {
	/// The archetype's current generation to distinguish it between other archetypes in the same
	/// slot. Entities are cleaned up once their entity count drops to zero.
	gen: ArchGen,

	/// The storages contained within this archetype.
	storages: Box<[StorageId]>,

	/// Entity IDs in this archetype.
	entity_ids: Vec<Entity>,

	/// ...and their corresponding dirty-tracking metadata nodes.
	entity_dirty_meta: Vec<DirtyEntityNode>,

	/// The last dirty element index.
	dirty_head: OptionalUsize,

	/// The last dirty element's ID for a quicker fast-path.
	dirty_id_gen: DirtyId,
}

impl ArchetypeData {
	fn mark_dirty(&mut self) {
		todo!()
	}

	fn iter_dirties(&mut self, last_checked: &mut u64) -> impl Iterator<Item = usize> {
		todo!()
	}
}

#[derive(Debug, Clone)]
struct DirtyEntityNode {
	dirty_id: u64,
	prev_dirty: OptionalUsize,
	next_dirty: OptionalUsize,
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
struct Conversion {
	low: ArchId,
	high: ArchId,
}

impl Conversion {
	pub fn new(a: ArchId, b: ArchId) -> Self {
		if a < b {
			Self { low: a, high: b }
		} else {
			Self { low: b, high: a }
		}
	}
}

// === World interface === //

impl World {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn spawn_now(&mut self) -> Entity {
		let gen = self.generation_gen.get_mut().try_generate().unwrap();
		let gen = NonZeroU64::new(gen).unwrap();

		let index = self.free_slots.pop_front();
		let index = match index {
			Some(index) => {
				self.slots[index].gen = Some(gen);
				index
			}
			None => {
				self.slots.push(EntitySlot {
					gen: Some(gen),
					arch: OptionalUsize::NONE,
					index_in_arch: 0,
				});
				self.slots.len() - 1
			}
		};

		Entity {
			_ty: PhantomData,
			index,
			gen,
		}
	}

	fn despawn_now_inner(&mut self, index: usize) {
		self.slots[index].gen = None;
		self.free_slots.push(index);
	}

	pub fn despawn_now(&mut self, target: Entity) -> Result<(), EntityDeadError> {
		// Validate slot
		let slot = self
			.slots
			.get_mut(target.index)
			.ok_or(EntityDeadError(target))?;

		if slot.gen != Some(target.gen) {
			return Err(EntityDeadError(target));
		}

		// Handle despawn
		self.despawn_now_inner(target.index);
		Ok(())
	}

	pub fn is_alive(&self, entity: Entity) -> bool {
		match self.slots.get(entity.index) {
			Some(slot) if slot.gen == Some(entity.gen) => true,
			_ => false,
		}
	}

	pub fn is_future_entity(&self, entity: Entity) -> bool {
		entity.gen > self.last_flushed_gen
	}

	pub fn flush(&mut self) {
		// === Create requested entities === //
		{
			let mut gen = self.last_flushed_gen;
			let max_gen = *self.generation_gen.get_mut();

			// Reserve slots in existing allocation
			while gen < max_gen && !self.free_slots.is_empty() {
				let index = self.free_slots.pop().unwrap();

				// Initialize the generation
				self.slots[index] = EntitySlot {
					gen: Some(NonZeroU64::new(gen).unwrap()),
					arch_index: OptionalUsize::NONE,
					comp_index: 0,
				};
			}

			// Reserve slots at end of buffer
			self.slots.reserve((max_gen - gen) as usize);
			for gen in gen..max_gen {
				let gen = NonZeroU64::new(gen).unwrap();

				self.slots.push(EntitySlot {
					gen: Some(gen),
					arch_index: OptionalUsize::NONE,
					comp_index: 0,
				});
			}
		}

		// Update `last_flushed_gen`
		self.last_flushed_gen = NonZeroU64::new(*self.generation_gen.get_mut()).unwrap();

		// === Honor action requests === //

		// Keeps track of the source archetypes of each modified entity and provides a convenient
		// iterator of modified entities for the update pass.
		let mut dirty_sources = HashMap::new();

		while let Some(st_action_queue) = self.mt_action_queue.pop() {
			let mut st_action_queue = st_action_queue.iter().copied();

			while let Some(base_cmd) = st_action_queue.next() {
				let base_target = base_cmd & !usize_msb_mask(0);

				if usize_has_mask(base_cmd, usize_msb_mask(0)) {
					// This is a deletion, handle it now.
					self.despawn_now_inner(base_target);
				} else {
					// This is an archetypal adjustment.
					let target = base_cmd;

					// ...
				}
			}
		}

		// === Adjust archetypes === //

		// ...
	}

	pub fn gen_storage_now(&mut self) -> StorageId {
		todo!()
	}

	pub fn attach_storage_now(&mut self, entity: Entity, storages: &[StorageId]) {
		todo!()
	}

	pub fn detach_storage_now(&mut self, entity: Entity, storages: &[StorageId]) {
		todo!()
	}

	pub fn iter_arch_changes(&mut self, last_updated: &mut DirtyId) -> impl Iterator<Item = usize> {
		todo!()
	}

	pub fn arch_entities(&self, index: usize) -> &[Entity] {
		todo!()
	}
}

pub type WorldHandleRef<'a> = WorldHandle<&'a World>;

#[derive(Debug, Clone)]
pub struct WorldHandle<H: Deref<Target = World>> {
	handle: H,
	st_action_queue: Vec<usize>,
}

impl<H: Deref<Target = World>> WorldHandle<H> {
	pub fn new(handle: H) -> Self {
		Self {
			handle,
			st_action_queue: Vec::new(),
		}
	}

	pub fn world(&self) -> &World {
		self.handle.deref()
	}

	pub fn handle(&self) -> &H {
		&self.handle
	}

	pub fn handle_mut(&mut self) -> &mut H {
		&mut self.handle
	}

	pub fn to_handle(self) -> H {
		self.handle
	}

	pub fn spawn_deferred(&mut self) -> Entity {
		// Determine generation
		let gen = (&self.generation_gen).try_generate().unwrap();
		let count = self.free_slots.len() as isize - (gen - self.last_flushed_gen) as isize;
		let gen = NonZeroU64::new(gen).unwrap();

		// Determine index
		let index = if count <= 0 {
			// When count is positive, it's an index in the free slot list.
			self.free_slots[count as usize]
		} else {
			// When it's negative, it's a number of entities allocated at the end of the free slot
			// list.
			self.slots.len() - 1 + (-count)
		};

		// Build an entity to add in the next flush.
		Entity {
			_ty: PhantomData,
			index,
			gen,
		}
	}

	pub fn despawn_deferred(&mut self, target: Entity) {
		// Proper checks will be done during flushing. We check this here in debug builds so the
		// errors are more apparent to the user.
		debug_assert!(self.is_alive(target) || self.is_future_entity(target));

		// indices will never be greater than `isize::MAX` so this is safe.
		self.st_action_queue.push(usize_msb_mask(0) | target.index);
	}

	pub fn gen_storage_deferred(&mut self) -> StorageId {
		todo!()
	}

	pub fn attach_storage_deferred(&mut self, entity: Entity, storages: &[StorageId]) {
		todo!()
	}

	pub fn detach_storage_deferred(&mut self, entity: Entity, storages: &[StorageId]) {
		todo!()
	}
}

impl<H: Deref<Target = World>> Deref for WorldHandle<H> {
	type Target = World;

	fn deref(&self) -> &Self::Target {
		self.world()
	}
}

impl<H: Deref<Target = World>> Drop for WorldHandle<H> {
	fn drop(&mut self) {
		self.world()
			.mt_action_queue
			.push(self.st_action_queue.into_boxed_slice());
	}
}

// === Entity types === //

#[derive_where(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct Entity<A = ()> {
	_ty: PhantomData<fn(A) -> A>,
	index: usize,
	gen: EntityGen,
}
