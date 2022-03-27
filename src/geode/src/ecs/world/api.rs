use crate::ecs::world::arch::{ArchManager, Archetype};
use crate::ecs::world::entities::{EntityDeadError, EntityManager};
use crate::ecs::world::ids::StorageId;
use crate::ecs::world::queue::{EntityActionEncoder, ReshapeAction};
use crate::ecs::world::{ArchHandle, ArchetypeDeadError, EntityArchLocator, EntityGen};
use crossbeam::queue::SegQueue;
use std::ops::{Deref, DerefMut};

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

#[derive(Debug, Default)]
pub struct World {
	entities: EntityManager,
	arch: ArchManager,
	delete_queues: SegQueue<Box<[usize]>>,
	reshape_queues: SegQueue<Box<[u64]>>,
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

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct Entity {
	pub(super) index: usize,
	pub(super) gen: EntityGen,
}

impl Entity {
	pub fn hash_index(&self) -> u64 {
		self.index as u64
	}

	pub fn index(&self) -> usize {
		self.index
	}

	pub fn gen(&self) -> EntityGen {
		self.gen
	}
}

#[derive(Debug, Copy, Clone)]
pub struct ComponentPair<'a, T: ?Sized> {
	pub entity: Entity,
	pub comp: &'a T,
}

impl<'a, T: ?Sized> Deref for ComponentPair<'a, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		self.comp
	}
}

#[derive(Debug)]
pub struct ComponentPairMut<'a, T: ?Sized> {
	pub entity: Entity,
	pub comp: &'a mut T,
}

impl<'a, T: ?Sized> ComponentPairMut<'a, T> {
	pub fn entity(&self) -> Entity {
		self.entity
	}

	pub fn component(&self) -> &T {
		self.comp
	}

	pub fn component_mut(&mut self) -> &mut T {
		self.comp
	}

	pub fn to_component(self) -> &'a mut T {
		self.comp
	}

	pub fn downgrade(&self) -> ComponentPair<'_, T> {
		ComponentPair {
			entity: self.entity,
			comp: self.comp,
		}
	}
}

impl<'a, T: ?Sized> Deref for ComponentPairMut<'a, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		self.comp
	}
}

impl<'a, T: ?Sized> DerefMut for ComponentPairMut<'a, T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		self.comp
	}
}
