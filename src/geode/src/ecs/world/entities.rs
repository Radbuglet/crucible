use super::{ids::EntityGenGenerator, Entity, EntityGen};
use crate::ecs::world::arch::RawEntityArchLocator;
use crate::util::number::{NumberGenMut, NumberGenRef};
use thiserror::Error;

#[derive(Debug)]
pub struct EntityManager {
	/// The generation generator at the time it was last flushed.
	gen_at_last_flush: EntityGen,

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
			gen_at_last_flush: EntityGen::new(1).unwrap(),
			generation_gen: EntityGenGenerator::default(),
			slots: Vec::new(),
			free_slots: Vec::new(),
		}
	}
}

impl EntityManager {
	pub fn spawn_now(&mut self) -> Entity {
		// Increment generation
		let gen = self.generation_gen.try_generate_mut().unwrap();
		self.gen_at_last_flush = gen;

		// Determine index
		let index = self.free_slots.pop();
		let index = match index {
			Some(index) => {
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
		let gen = self.generation_gen.try_generate_ref().unwrap();
		let count =
			self.free_slots.len() as isize - (gen.get() - self.gen_at_last_flush.get()) as isize;

		// Determine index
		let index = if count <= 0 {
			// When count is positive, it's an index in the free slot list.
			self.free_slots[count as usize]
		} else {
			// When it's negative, it's a number of entities allocated at the end of the free slot
			// list.
			self.slots.len() - 1 + (-count) as usize
		};

		// Build entity handle
		Entity { index, gen }
	}

	pub fn despawn_by_slot_now(&mut self, target: usize) -> Result<(), EntitySlotDeadError> {
		let slot = self
			.slots
			.get_mut(target)
			.ok_or(EntitySlotDeadError(target))?;

		*slot = None;
		self.free_slots.push(target);
		Ok(())
	}

	pub fn despawn_now(&mut self, target: Entity) -> Result<(), EntityDeadError> {
		if self.is_alive(target) {
			self.despawn_by_slot_now(target.index).unwrap();
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
		entity.gen >= self.gen_at_last_flush
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
		let max_gen = self.generation_gen.next_value();
		let mut gen = self.gen_at_last_flush.get();

		// Reserve slots in existing allocation
		while gen < max_gen.get() && !self.free_slots.is_empty() {
			let index = self.free_slots.pop().unwrap();
			self.slots[index] = Some(EntitySlot::new(EntityGen::new(gen).unwrap()));
			gen += 1;
		}

		// Reserve slots at end of buffer
		self.slots.reserve((max_gen.get() - gen) as usize);
		for gen in gen..max_gen.get() {
			self.slots
				.push(Some(EntitySlot::new(EntityGen::new(gen).unwrap())));
		}

		// Update `last_flushed_gen`
		self.gen_at_last_flush = max_gen;
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
