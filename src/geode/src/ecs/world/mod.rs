use crossbeam::queue::SegQueue;
use derive_where::derive_where;
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::atomic::AtomicU64;

// === Internal modules === //

mod arch;
mod entities;
mod queue;

use arch::{ArchManager, Archetype, ArchetypeDeadError};
use entities::EntityManager;
use queue::{EntityActionEncoder, ReshapeAction};

// === Re-exports === //

pub use arch::{ArchHandle, EntityArchLocator};
pub use entities::EntityDeadError;

// === Identifiers === //

// TODO: Use non-zero types for these identifiers to allow for better niche optimizations.

/// An entity generation; used to distinguish between multiple distinct entities in a single slot.
pub type EntityGen = u64;
type AtomicEntityGen = AtomicU64;

/// The unique identifier of a storage.
type StorageId = u64;
type AtomicStorageId = AtomicU64;

/// An archetype generation; used to distinguish between multiple distinct archetypes in a single slot.
pub type ArchGen = u64;

/// An identifier for a snapshot in the archetype's history. Used to lazily bring storages up-to-date.
pub type DirtyId = u64;

// === World === //

#[derive(Debug, Default)]
pub struct World {
	entities: EntityManager,
	arch: ArchManager,
	delete_queues: SegQueue<Box<[usize]>>,
	reshape_queues: SegQueue<Box<[u64]>>,
}

pub trait WorldAccessor {
	fn raw_world(&self) -> &World;

	fn is_sync(&self) -> bool;

	fn spawn(&mut self) -> Entity;

	fn despawn(&mut self, target: Entity);

	fn is_alive(&self, target: Entity) -> bool {
		self.raw_world().is_alive(target)
	}

	fn is_future_entity(&self, target: Entity) -> bool {
		self.raw_world().is_future_entity(target)
	}

	fn is_alive_or_future(&self, target: Entity) -> bool {
		self.raw_world().is_alive_or_future(target)
	}

	fn locate_entity(&self, target: Entity) -> Result<Option<EntityArchLocator>, EntityDeadError> {
		self.raw_world().locate_entity(target)
	}

	fn get_archetype(&self, handle: ArchHandle) -> Result<&Archetype, ArchetypeDeadError> {
		self.raw_world().get_archetype(handle)
	}

	fn find_archetype<I>(&self, comp_list_sorted: I) -> Option<ArchHandle>
	where
		I: IntoIterator<Item = StorageId>,
		I::IntoIter: ExactSizeIterator + Clone,
	{
		self.raw_world().find_archetype(comp_list_sorted)
	}

	fn new_storage(&mut self) -> StorageId;

	fn attach_storage(&mut self, target: Entity, storage: StorageId);

	fn detach_storage(&mut self, target: Entity, storage: StorageId);
}

impl World {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn ref_handle(&self) -> AsyncWorldHandleRef<'_> {
		AsyncWorldHandle::new(self)
	}

	pub fn flush(&mut self) {
		// Spawn semi-alive entities
		self.entities.flush_creations();

		// Flush despawn requests
		while let Some(queue) = self.delete_queues.pop() {
			for target in queue.iter().copied() {
				let result = self.entities.despawn_by_slot_now(target);
				debug_assert!(result.is_ok());
			}
		}

		// Flush reshape requests
		todo!()
	}
}

impl WorldAccessor for World {
	fn raw_world(&self) -> &World {
		self
	}

	fn is_sync(&self) -> bool {
		true
	}

	fn spawn(&mut self) -> Entity {
		self.entities.spawn_now()
	}

	fn despawn(&mut self, target: Entity) {
		self.entities.despawn_now(target).unwrap();
	}

	fn is_alive(&self, target: Entity) -> bool {
		self.entities.is_alive(target)
	}

	fn is_future_entity(&self, target: Entity) -> bool {
		self.entities.is_future_entity(target)
	}

	fn is_alive_or_future(&self, target: Entity) -> bool {
		self.entities.is_alive_or_future(target)
	}

	fn locate_entity(&self, target: Entity) -> Result<Option<EntityArchLocator>, EntityDeadError> {
		if self.is_alive(target) {
			let (_, arch_raw) = self.entities.locate_entity_raw(target.index);
			Ok(arch_raw
				.arch_index
				.as_option()
				.map(|slot| EntityArchLocator {
					arch: self.arch.slot_to_handle(slot),
					index_in_arch: arch_raw.index_in_arch,
				}))
		} else {
			Err(EntityDeadError(target))
		}
	}

	fn get_archetype(&self, handle: ArchHandle) -> Result<&Archetype, ArchetypeDeadError> {
		self.arch.get_arch(handle)
	}

	fn find_archetype<I>(&self, comp_list_sorted: I) -> Option<ArchHandle>
	where
		I: IntoIterator<Item = StorageId>,
		I::IntoIter: ExactSizeIterator + Clone,
	{
		self.arch.find_arch(comp_list_sorted)
	}

	fn new_storage(&mut self) -> StorageId {
		self.arch.new_storage_sync()
	}

	fn attach_storage(&mut self, target: Entity, storage: StorageId) {
		assert!(self.is_alive(target));

		// Get entity slot info
		let (_, slot) = self.entities.locate_entity_raw(target.index);

		// Find the target archetype
		let arch = self.arch.arch_dest_for_addition(slot.arch_index, storage);

		// Move from the old archetype to the new one
		#[rustfmt::skip]
		self.arch.move_to_arch(&mut self.entities, target.index, arch);
	}

	fn detach_storage(&mut self, target: Entity, storage: StorageId) {
		assert!(self.is_alive(target));

		// Get entity slot info
		let (_, slot) = self.entities.locate_entity_raw(target.index);

		// Find the target archetype
		let arch = self.arch.arch_dest_for_deletion(slot.arch_index, storage);

		// Move from the old archetype to the new one
		#[rustfmt::skip]
		self.arch.move_to_arch(&mut self.entities, target.index, arch);
	}
}

pub type AsyncWorldHandleRef<'a> = AsyncWorldHandle<&'a World>;

#[derive(Debug)]
pub struct AsyncWorldHandle<H: Deref<Target = World>> {
	handle: H,
	queues: Option<Queues>,
}

#[derive(Debug, Default)]
struct Queues {
	reshape_queue: EntityActionEncoder,
	deletion_queue: Vec<usize>,
}

impl<H: Deref<Target = World>> AsyncWorldHandle<H> {
	pub fn new(handle: H) -> Self {
		Self {
			handle,
			queues: Some(Queues::default()),
		}
	}

	pub fn handle(&self) -> &H {
		&self.handle
	}

	fn queues(&mut self) -> &mut Queues {
		self.queues.as_mut().unwrap()
	}
}

impl<H: Deref<Target = World>> WorldAccessor for AsyncWorldHandle<H> {
	fn raw_world(&self) -> &World {
		self.handle.deref()
	}

	fn is_sync(&self) -> bool {
		false
	}

	fn spawn(&mut self) -> Entity {
		self.handle.entities.spawn_deferred()
	}

	fn despawn(&mut self, target: Entity) {
		debug_assert!(self.is_alive_or_future(target));
		self.queues().deletion_queue.push(target.index);
	}

	fn new_storage(&mut self) -> StorageId {
		self.handle.arch.new_storage_async()
	}

	fn attach_storage(&mut self, target: Entity, storage: StorageId) {
		self.queues().reshape_queue.add(ReshapeAction::Add {
			slot: target.index,
			storage,
		});
	}

	fn detach_storage(&mut self, target: Entity, storage: StorageId) {
		self.queues().reshape_queue.add(ReshapeAction::Remove {
			slot: target.index,
			storage,
		});
	}
}

impl<H: Deref<Target = World>> Drop for AsyncWorldHandle<H> {
	fn drop(&mut self) {
		let queues = self.queues.take().unwrap();

		self.raw_world()
			.reshape_queues
			.push(queues.reshape_queue.finish());

		self.raw_world()
			.delete_queues
			.push(queues.deletion_queue.into_boxed_slice());
	}
}

// === Entity === //

#[derive_where(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct Entity<A = ()> {
	_ty: PhantomData<fn(A) -> A>,
	index: usize,
	gen: EntityGen,
}
