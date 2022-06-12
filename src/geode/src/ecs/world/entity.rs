use crate::util::number::{AtomicNZU64Generator, NonZeroU64Generator, NumberGenMut, NumberGenRef};
use crossbeam::queue::SegQueue;
use derive_where::derive_where;
use std::num::NonZeroU64;

use super::Entity;

#[derive(Debug)]
#[derive_where(Default)]
pub struct EntityManager<M> {
	// Stores the entity slots allocated within the world.
	// Useful for checking if an entity is alive and quickly
	// locating its position in an archetype.
	entities: Vec<Option<EntitySlot<M>>>,

	// A list of free entity slots at the last flush. Free entities
	// are yielded in a LIFO manner.
	free_entities: Vec<usize>,

	// An atomic counter for the generation of the next entity to be
	// spawned.
	gen_generator: AtomicNZU64Generator,

	// The highest entity generation at the last world flush.
	last_flush_max_gen: u64,

	// A queue of deletion requests.
	deletions: SegQueue<Box<[usize]>>,
}

#[derive(Debug)]
struct EntitySlot<M> {
	gen: NonZeroU64,
	meta: M,
}

impl<M: Default> EntityManager<M> {
	pub fn spawn_now(&mut self) -> Entity {
		let entity = self.queue_spawn();

		// Flush entities to spawn target immediately. Flushing isn't unexpected
		// behavior because the handles capable of queued world operations will
		// typically flush the world on `Drop`.
		self.flush();

		entity
	}

	pub fn despawn_now(&mut self, target: Entity) -> M {
		// See `despawn_now.
		self.flush();

		let slot = &mut self.entities[target.slot];

		// We ensure that the slot matches the target entity before manipulating
		// state to make this object more panic-safe.
		assert_eq!(slot.as_ref().unwrap().gen, target.gen);

		self.free_entities.push(target.slot);
		slot.take().unwrap().meta
	}

	pub fn queue_spawn(&self) -> Entity {
		let gen = self.gen_generator.generate_ref();

		// Get the number of entities spawned since the last flush.
		let spawned = gen.get() - self.last_flush_max_gen;
		let spawned = spawned as isize;

		// Get the index of the slot from which we'll take our free entity slot.
		let index_in_free_vec = self.free_entities.len() as isize - spawned;

		// Derive the slot of our new entity
		let slot = if index_in_free_vec >= 0 {
			self.free_entities[index_in_free_vec as usize]
		} else {
			let end_index = self.entities.len();
			let offset = (-index_in_free_vec) as usize - 1;
			end_index + offset
		};

		Entity { slot, gen }
	}

	pub fn queue_despawn_many(&self, entities: Box<[usize]>) {
		self.deletions.push(entities);
	}

	pub fn get_meta(&self, target: Entity) -> Option<&M> {
		self.entities.get(target.slot).and_then(|slot| {
			let slot = slot.as_ref()?;
			if slot.gen == target.gen {
				Some(&slot.meta)
			} else {
				None
			}
		})
	}

	pub fn get_meta_mut(&mut self, target: Entity) -> Option<&mut M> {
		self.entities.get_mut(target.slot).and_then(|slot| {
			let slot = slot.as_mut()?;
			if slot.gen == target.gen {
				Some(&mut slot.meta)
			} else {
				None
			}
		})
	}

	pub fn is_alive_now(&self, target: Entity) -> bool {
		self.get_meta(target).is_some()
	}

	pub fn is_future(&self, target: Entity) -> bool {
		target.gen.get() > self.last_flush_max_gen
	}

	pub fn is_not_condemned(&self, target: Entity) -> bool {
		self.is_alive_now(target) || self.is_future(target)
	}

	pub fn is_condemned(&self, target: Entity) -> bool {
		!self.is_not_condemned(target)
	}

	pub fn state_of(&self, target: Entity) -> EntityState {
		if self.is_alive_now(target) {
			EntityState::Alive
		} else if self.is_future(target) {
			EntityState::Future
		} else {
			EntityState::Condemned
		}
	}

	pub fn flush(&mut self) {
		// We handle spawn requests first so despawns don't move stuff around
		// and despawns of nursery entities can be honored.
		{
			let first_gen_id = NonZeroU64::new(self.last_flush_max_gen + 1).unwrap();
			let mut id_gen = NonZeroU64Generator { next: first_gen_id };
			let max_gen_exclusive = self.gen_generator.next_value();

			// Handle slot reuses
			while id_gen.next < max_gen_exclusive && !self.free_entities.is_empty() {
				let slot = self.free_entities.pop().unwrap();
				self.entities[slot] = Some(EntitySlot {
					gen: id_gen.generate_mut(),
					meta: M::default(),
				});
			}

			// Handle slot pushes
			let remaining = max_gen_exclusive.get() - id_gen.next.get();
			let iter = std::iter::repeat_with(|| {
				Some(EntitySlot {
					gen: id_gen.generate_mut(),
					meta: M::default(),
				})
			})
			.take(remaining as usize);

			self.entities.extend(iter);
			self.last_flush_max_gen = self.gen_generator.next_value().get() - 1;
		}

		// Now, handle despawn requests.
		while let Some(slots) = self.deletions.pop() {
			for slot_idx in slots.iter().copied() {
				let slot = &mut self.entities[slot_idx];
				if slot.is_none() {
					continue;
				}
				*slot = None;
				self.free_entities.push(slot_idx);
			}
		}
	}
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum EntityState {
	Alive,
	Future,
	Condemned,
}

#[cfg(test)]
mod tests {
	use crate::util::test_utils::{init_seed, rand_choice};

	use super::*;
	use indexmap::IndexSet;
	use std::collections::HashSet;

	fn random_elem<T>(set: &IndexSet<T>) -> &T {
		set.get_index(fastrand::usize(0..set.len())).unwrap()
	}

	#[derive(Debug, Default)]
	struct Simulator {
		all: HashSet<Entity>,
		alive: IndexSet<Entity>,
		staged_add: IndexSet<Entity>,
		staged_remove: Vec<Entity>,
	}

	impl Simulator {
		pub fn queue_spawn(&mut self, target: Entity) {
			assert!(!self.all.contains(&target));
			self.all.insert(target);
			self.staged_add.insert(target);
		}

		pub fn queue_despawn(&mut self, target: Entity) {
			assert!(self.all.contains(&target));
			self.staged_remove.push(target);
		}

		pub fn state_of(&self, target: Entity) -> EntityState {
			if self.alive.contains(&target) {
				EntityState::Alive
			} else if self.staged_add.contains(&target) {
				EntityState::Future
			} else {
				EntityState::Condemned
			}
		}

		pub fn flush(&mut self) {
			for add in self.staged_add.drain(..) {
				self.alive.insert(add);
			}

			for removed in self.staged_remove.drain(..) {
				self.alive.remove(&removed);
			}
		}

		pub fn random_entity(&self) -> Option<Entity> {
			rand_choice! {
				!self.alive.is_empty() => Some(*random_elem(&self.alive)),
				!self.staged_add.is_empty() => Some(*random_elem(&self.staged_add)),
				_ => None,
			}
		}

		pub fn assert_eq_to<M: Default>(&self, mgr: &EntityManager<M>) {
			for entity in self.all.iter().copied() {
				assert_eq!(self.state_of(entity), mgr.state_of(entity));
			}
		}
	}

	#[test]
	fn auto_entity_test() {
		init_seed();

		let mut manager: EntityManager<()> = Default::default();
		let mut simulator = Simulator::default();

		for i in 0..1000 {
			println!("Stage {i}");

			for _ in 0..fastrand::u32(0..10) {
				let random_entity = simulator.random_entity();
				rand_choice! {
					true => {
						let entity = manager.queue_spawn();
						simulator.queue_spawn(entity);
						println!("Spawning {:?}", entity);
					},
					random_entity.is_some() => {
						let target = random_entity.unwrap();
						println!("Despawning {:?}", target);
						simulator.queue_despawn(target);
						manager.queue_despawn_many(Box::new([target.slot()]));
					},
					_ => unreachable!(),
				};
			}

			manager.flush();
			simulator.flush();

			if i % 5 == 0 {
				simulator.assert_eq_to(&manager);
			}
		}
	}
}
