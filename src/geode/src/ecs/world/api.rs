use crate::ecs::world::arch::{ArchManager, WorldArchetype};
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
		// TODO: Pass event to ArchManager as well.
		self.entities.despawn_by_slot_now(target.index).unwrap();
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

	pub fn get_archetype(&self, handle: ArchHandle) -> Result<&WorldArchetype, ArchetypeDeadError> {
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
		self.arch.new_storage()
	}

	pub fn attach_storage_now(&mut self, target: Entity, storage: StorageId) {
		assert!(self.is_alive(target));

		// Get entity slot info
		let (_, slot) = self.entities.locate_entity_raw(target.index);

		// Find the target archetype
		let arch = self.arch.arch_dest_for_addition(slot.arch_index, storage);

		// Move from the old archetype to the new one
		#[rustfmt::skip]
		self.arch.move_to_arch_and_track_locs(&mut self.entities, target.index, arch);
	}

	pub fn detach_storage_now(&mut self, target: Entity, storage: StorageId) {
		assert!(self.is_alive(target));

		// Get entity slot info
		let (_, slot) = self.entities.locate_entity_raw(target.index);

		// Find the target archetype
		let arch = self.arch.arch_dest_for_deletion(slot.arch_index, storage);

		// Move from the old archetype to the new one
		#[rustfmt::skip]
		self.arch.move_to_arch_and_track_locs(&mut self.entities, target.index, arch);
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
		self.handle.arch.new_storage_multi_threaded()
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
	pub fn index(&self) -> usize {
		self.index
	}

	pub fn gen(&self) -> EntityGen {
		self.gen
	}
}

#[derive(Debug, Copy, Clone)]
pub struct ComponentPair<'a, T: ?Sized> {
	entity: Entity,
	comp: &'a T,
}

impl<'a, T: ?Sized> ComponentPair<'a, T> {
	pub fn new(entity: Entity, comp: &'a T) -> Self {
		Self { entity, comp }
	}

	pub fn entity(this: &Self) -> Entity {
		this.entity
	}

	pub fn comp(this: &Self) -> &'a T {
		this.comp
	}
}

impl<'a, T: ?Sized> Deref for ComponentPair<'a, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		self.comp
	}
}

#[derive(Debug)]
pub struct ComponentPairMut<'a, T: ?Sized> {
	entity: Entity,
	comp: &'a mut T,
}

impl<'a, T: ?Sized> ComponentPairMut<'a, T> {
	pub fn new(entity: Entity, comp: &'a mut T) -> Self {
		Self { entity, comp }
	}

	pub fn entity(this: &Self) -> Entity {
		this.entity
	}

	pub fn component(this: &Self) -> &T {
		this.comp
	}

	pub fn component_mut(this: &mut Self) -> &mut T {
		this.comp
	}

	pub fn to_component(this: Self) -> &'a mut T {
		this.comp
	}

	pub fn downgrade(this: &Self) -> ComponentPair<'_, T> {
		ComponentPair {
			entity: this.entity,
			comp: this.comp,
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
