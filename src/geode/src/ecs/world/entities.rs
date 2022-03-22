use super::{AtomicEntityGen, Entity, EntityGen};
use crate::ecs::world::arch::RawEntityArchLocator;
use crate::util::number::NumberGenExt;
use std::marker::PhantomData;
use std::sync::atomic::AtomicU64;
use thiserror::Error;

#[derive(Debug)]
pub struct EntityManager {
	/// The generation generator at the time it was last flushed.
	gen_at_last_flush: EntityGen,

	/// A monotonically increasing generation counter. This represents the next value to be yielded
	/// when generating new generations.
	generation_gen: AtomicEntityGen,

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
			// We start generators at `1` instead of `0` in preparation for our move to `NonZeroU64`.
			gen_at_last_flush: 1,
			generation_gen: AtomicU64::new(1),
			slots: Vec::new(),
			free_slots: Vec::new(),
		}
	}
}

impl EntityManager {
	pub fn spawn_now(&mut self) -> Entity {
		// Increment generation
		let gen = self.generation_gen.get_mut().try_generate().unwrap();
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

		Entity {
			_ty: PhantomData,
			index,
			gen,
		}
	}

	pub fn spawn_deferred(&self) -> Entity {
		// Determine generation
		let gen = (&self.generation_gen).try_generate().unwrap();
		let count = self.free_slots.len() as isize - (gen - self.gen_at_last_flush) as isize;

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
		Entity {
			_ty: PhantomData,
			index,
			gen,
		}
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
		let max_gen = *self.generation_gen.get_mut();
		let mut gen = self.gen_at_last_flush;

		// Reserve slots in existing allocation
		while gen < max_gen && !self.free_slots.is_empty() {
			let index = self.free_slots.pop().unwrap();
			self.slots[index] = Some(EntitySlot::new(gen));
			gen += 1;
		}

		// Reserve slots at end of buffer
		self.slots.reserve((max_gen - gen) as usize);
		for gen in gen..max_gen {
			self.slots.push(Some(EntitySlot::new(gen)));
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
