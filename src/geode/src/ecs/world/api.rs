use crate::ecs::world::arch::{ArchManager, Archetype};
use crate::ecs::world::entities::{EntityDeadError, EntityManager};
use crate::ecs::world::ids::StorageId;
use crate::ecs::world::queue::{EntityActionEncoder, ReshapeAction};
use crate::ecs::world::{ArchHandle, ArchetypeDeadError, EntityArchLocator, EntityGen};
use crossbeam::queue::SegQueue;
use std::ops::{Deref, DerefMut};

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

	pub fn queue_ref(&self) -> WorldQueueRef<'_> {
		WorldQueue::new(self)
	}

	pub fn spawn_now(&mut self) -> Entity {
		self.entities.spawn_now()
	}

	pub fn despawn_now(&mut self, target: Entity) {
		self.entities.despawn_now(target).unwrap();
	}

	pub fn is_alive(&self, target: Entity) -> bool {
		self.entities.is_alive(target)
	}

	pub fn is_future_entity(&self, target: Entity) -> bool {
		self.entities.is_future_entity(target)
	}

	pub fn is_alive_or_future(&self, target: Entity) -> bool {
		self.entities.is_alive_or_future(target)
	}

	pub fn locate_entity(
		&self,
		target: Entity,
	) -> Result<Option<EntityArchLocator>, EntityDeadError> {
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

	pub fn get_archetype(&self, handle: ArchHandle) -> Result<&Archetype, ArchetypeDeadError> {
		self.arch.get_arch(handle)
	}

	pub fn find_archetype<I>(&self, comp_list_sorted: I) -> Option<ArchHandle>
	where
		I: IntoIterator<Item = StorageId>,
		I::IntoIter: ExactSizeIterator + Clone,
	{
		self.arch.find_arch(comp_list_sorted)
	}

	pub fn new_storage_now(&mut self) -> StorageId {
		self.arch.new_storage_sync()
	}

	pub fn attach_storage_now(&mut self, target: Entity, storage: StorageId) {
		assert!(self.is_alive(target));

		// Get entity slot info
		let (_, slot) = self.entities.locate_entity_raw(target.index);

		// Find the target archetype
		let arch = self.arch.arch_dest_for_addition(slot.arch_index, storage);

		// Move from the old archetype to the new one
		#[rustfmt::skip]
		self.arch.move_to_arch(&mut self.entities, target.index, arch);
	}

	pub fn detach_storage_now(&mut self, target: Entity, storage: StorageId) {
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

pub type WorldQueueRef<'a> = WorldQueue<&'a World>;

#[derive(Debug)]
pub struct WorldQueue<H: Deref<Target = World>> {
	handle: H,
	queues: Option<Queues>,
}

#[derive(Debug, Default)]
struct Queues {
	reshape_queue: EntityActionEncoder,
	deletion_queue: Vec<usize>,
}

impl<H: Deref<Target = World>> WorldQueue<H> {
	pub fn new(handle: H) -> Self {
		Self {
			handle,
			queues: Some(Queues::default()),
		}
	}

	fn queues(&mut self) -> &mut Queues {
		self.queues.as_mut().unwrap()
	}

	pub fn handle(&self) -> &H {
		&self.handle
	}

	pub fn spawn_deferred(&mut self) -> Entity {
		self.handle.entities.spawn_deferred()
	}

	pub fn despawn_deferred(&mut self, target: Entity) {
		debug_assert!(self.is_alive_or_future(target));
		self.queues().deletion_queue.push(target.index);
	}

	pub fn new_storage_async(&mut self) -> StorageId {
		self.handle.arch.new_storage_async()
	}

	pub fn attach_storage_deferred(&mut self, target: Entity, storage: StorageId) {
		self.queues().reshape_queue.add(ReshapeAction::Add {
			slot: target.index,
			storage,
		});
	}

	pub fn detach_storage_deferred(&mut self, target: Entity, storage: StorageId) {
		self.queues().reshape_queue.add(ReshapeAction::Remove {
			slot: target.index,
			storage,
		});
	}
}

impl<H: Deref<Target = World>> Deref for WorldQueue<H> {
	type Target = World;

	fn deref(&self) -> &Self::Target {
		&*self.handle
	}
}

impl<H: Deref<Target = World>> Drop for WorldQueue<H> {
	fn drop(&mut self) {
		let queues = self.queues.take().unwrap();

		self.handle
			.reshape_queues
			.push(queues.reshape_queue.finish());

		self.handle
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
