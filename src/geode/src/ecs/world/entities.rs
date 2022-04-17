use super::{ids::EntityGenGenerator, Entity, EntityGen};
use crate::ecs::world::arch::RawEntityArchLocator;
use crate::util::error::ResultExt;
use crate::util::number::{NumberGenMut, NumberGenRef};
use thiserror::Error;

#[derive(Debug)]
pub struct EntityManager {
	/// The generation generator at the time it was last flushed.
	next_gen_at_last_flush: EntityGen,

	/// A monotonically increasing generation counter. This represents the next value to be yielded
	/// when generating new generations.
	generation_gen: EntityGenGenerator,

	/// A buffer of alive entity slots or `None`. Never shrinks. Used to check for liveness and to
	/// find containing archetypes for archetype moving.
	slots: Vec<Option<EntitySlot>>,

	/// A list of all the free slots; only updated every flush. Used to find empty slots to reuse
	/// during both async and sync entity allocation.
	free_slots: Vec<usize>,
}

impl Default for EntityManager {
	fn default() -> Self {
		Self {
			next_gen_at_last_flush: EntityGen::new(1).unwrap(),
			generation_gen: EntityGenGenerator::default(),
			slots: Vec::new(),
			free_slots: Vec::new(),
		}
	}
}

impl EntityManager {
	pub fn spawn_now(&mut self) -> Entity {
		// Increment generation
		let gen = self.generation_gen.try_generate_mut().unwrap_pretty();
		self.next_gen_at_last_flush = self.generation_gen.next_value();

		// Determine index
		let index = self.free_slots.pop();
		let index = match index {
			Some(index) => {
				debug_assert!(self.slots[index].is_none());
				self.slots[index] = Some(EntitySlot::new(gen));
				index
			}
			None => {
				self.slots.push(Some(EntitySlot::new(gen)));
				self.slots.len() - 1
			}
		};

		Entity { index, gen }
	}

	pub fn spawn_deferred(&self) -> Entity {
		// Determine generation
		let gen = self.generation_gen.try_generate_ref().unwrap_pretty();

		// Determine the index of the entity being generated in the `free_slots` list if we were reading
		// from left to right.
		//
		// N.B. `next_gen_at_last_flush` represents the generation that would have been yielded by a
		// deferred spawn right after the flush. e.g. the first generation would result in
		// `gen == next_gen_at_last_flush` and a `gen_index` equal to `0`.
		//
		// This index may overflow a `usize` but that is fine because:
		// 1. We don't actually use these to handle flushing so internal invariants are not broken.
		// 2. Users can produce invalid entity IDs safely by using entity IDs from other worlds so
		//    it's not really worth guarding against bizarre behavior so long as it doesn't cause
		//    unsoundness.
		let free_list_index = (gen.get() - self.next_gen_at_last_flush.get()) as usize;

		// Determine index
		let index = if free_list_index < self.free_slots.len() {
			self.free_slots[self.free_slots.len() - 1 - free_list_index]
		} else {
			self.slots.len() + free_list_index
		};

		// Build entity handle
		Entity { index, gen }
	}

	pub fn despawn_by_slot_now(&mut self, target: usize) -> Result<(), EntitySlotDeadError> {
		let slot = self
			.slots
			.get_mut(target)
			// We need to ensure that the entity has not already been released lest queued deletions
			// add the same entity slot to the `free_slots` vector several times.
			.filter(|target| target.is_some())
			.ok_or(EntitySlotDeadError(target))?;

		*slot = None;
		self.free_slots.push(target);
		Ok(())
	}

	pub fn despawn_now(&mut self, target: Entity) -> Result<(), EntityDeadError> {
		if self.is_alive(target) {
			self.despawn_by_slot_now(target.index).unwrap_pretty();
			Ok(())
		} else {
			Err(EntityDeadError(target))
		}
	}

	pub fn is_alive(&self, entity: Entity) -> bool {
		match self.slots.get(entity.index) {
			Some(Some(slot)) if slot.gen == entity.gen => true,
			_ => false,
		}
	}

	pub fn is_future_entity(&self, entity: Entity) -> bool {
		entity.gen >= self.next_gen_at_last_flush
	}

	pub fn is_alive_or_future(&self, entity: Entity) -> bool {
		self.is_future_entity(entity) || self.is_alive(entity)
	}

	pub fn locate_entity_raw(&self, index: usize) -> (EntityGen, &RawEntityArchLocator) {
		let slot = self.slots[index].as_ref().unwrap();
		(slot.gen, &slot.arch)
	}

	pub fn locate_entity_raw_mut(
		&mut self,
		index: usize,
	) -> (EntityGen, &mut RawEntityArchLocator) {
		let slot = self.slots[index].as_mut().unwrap();
		(slot.gen, &mut slot.arch)
	}

	pub fn flush_creations(&mut self) {
		// The generation of the next entity this routine will spawn.
		let mut gen_of_next_spawn = self.next_gen_at_last_flush.get();

		// The next generation to be spawned by the global generation counter.
		// i.e. this is an upper exclusive bound for the generation.
		let global_next_gen = self.generation_gen.next_value();

		// Reserve slots in existing allocation
		while gen_of_next_spawn < global_next_gen.get() && !self.free_slots.is_empty() {
			let index = self.free_slots.pop().unwrap();
			self.slots[index] = Some(EntitySlot::new(EntityGen::new(gen_of_next_spawn).unwrap()));
			gen_of_next_spawn += 1;
		}

		// Reserve slots at end of buffer
		let extensions =
			// `global_next_gen` is an exclusive upper bound for generating entities
			(gen_of_next_spawn..global_next_gen.get())
				// Make each of the generations for which we're creating an entity into an entity slot.
				.map(|gen| {
					Some(EntitySlot::new(EntityGen::new(gen).unwrap()))
				});

		self.slots.extend(extensions);

		// Update `next_gen_at_last_flush`
		self.next_gen_at_last_flush = global_next_gen;
	}
}

#[derive(Debug, Clone)]
struct EntitySlot {
	/// The generation of the entity which is currently living in this slot.
	gen: EntityGen,

	/// The location of the archetype in which the entity is stored.
	arch: RawEntityArchLocator,
}

impl EntitySlot {
	pub fn new(gen: EntityGen) -> Self {
		Self {
			gen,
			arch: RawEntityArchLocator::default(),
		}
	}
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Error)]
#[error("entity {0:?} is dead")]
pub struct EntityDeadError(pub Entity);

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Error)]
#[error("entity at index {0} is dead")]
pub struct EntitySlotDeadError(pub usize);
