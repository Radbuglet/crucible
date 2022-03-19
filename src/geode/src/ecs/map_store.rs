use crate::ecs::entity::ComponentPairMut;
use crate::ecs::world::{Entity, WorldAccessor};
use hashbrown::raw::{Bucket, RawIter, RawTable};
use std::mem::replace;

// === MapStorage === //

pub struct MapStorage<T> {
	map: RawTable<(Entity, T)>,
}

impl<T> Default for MapStorage<T> {
	fn default() -> Self {
		Self {
			map: Default::default(),
		}
	}
}

impl<T> MapStorage<T> {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn insert(&mut self, world: &impl WorldAccessor, entity: Entity, value: T) -> Option<T> {
		assert!(world.is_alive(entity));

		// Try to find and replace the existing entity in the map.
		if let Some(slot) = self.map.get_mut(entity.hash_index(), |(candidate, _)| {
			// We can do this reduced equality check because two entities in the same slot but with
			// different generations will never be simultaneously alive in a single container.
			//
			// Doing this reduced check has the benefit of enabling searches to short-circuit
			// earlier and allows us to tombstone earlier versions of the same slot automatically.
			candidate.index == entity.index
		}) {
			let (replaced_entity, replaced_comp) = replace(slot, (entity, value));
			if replaced_entity.gen == entity.gen {
				Some(replaced_comp)
			} else {
				None
			}
		} else {
			let _ = self.force_insert(world, entity, value);
			None
		}
	}

	fn force_insert(&mut self, world: &World, entity: Entity, value: T) -> Bucket<(Entity, T)> {
		// The key isn't yet mapped to anything. We have to insert a new entry.

		// Try a simple insertion
		// We do this in two passes because we can't inject custom cleanup logic into the rehasher so
		// this is our best alternative.
		match self
			.map
			.try_insert_no_grow(entity.hash_index(), (entity, value))
		{
			// Insertion was successful.
			Ok(bucket) => bucket,

			// We need to grow.
			Err((entity, value)) => {
				// If we failed to clean up the existing chain, we are pretty much forced to grow.
				self.clean(world);
				self.map
					.insert(entity.hash_index(), (entity, value), |(rehashed, _)| {
						rehashed.hash_index()
					})
			}
		}
	}

	pub fn try_get_raw(&self, entity: Entity) -> Option<ComponentPair<'_, T>> {
		self.map
			.get(entity.hash_index(), |(candidate, _)| {
				// See reasoning above.
				candidate.index == entity.index
			})
			.and_then(|(found, comp)| {
				if entity.gen == found.gen {
					Some(ComponentPair { entity, comp })
				} else {
					None
				}
			})
	}

	pub fn get_raw(&self, entity: Entity) -> ComponentPair<'_, T> {
		self.try_get_raw(entity).unwrap()
	}

	pub fn try_get_mut_raw(&mut self, entity: Entity) -> Option<ComponentPairMut<'_, T>> {
		self.map
			.get_mut(entity.hash_index(), |(candidate, _)| {
				// See reasoning above.
				candidate.index == entity.index
			})
			.and_then(|(found, comp)| {
				if entity.gen == found.gen {
					Some(ComponentPairMut { entity, comp })
				} else {
					None
				}
			})
	}

	pub fn get_mut_raw(&mut self, entity: Entity) -> ComponentPairMut<'_, T> {
		self.try_get_mut_raw(entity).unwrap()
	}

	pub fn try_get(&self, world: &World, entity: Entity) -> Option<ComponentPair<'_, T>> {
		if world.is_alive(entity) {
			self.try_get_raw(entity)
		} else {
			None
		}
	}

	pub fn get(&self, world: &World, entity: Entity) -> ComponentPair<'_, T> {
		self.try_get(world, entity).unwrap()
	}

	pub fn get_or_insert<F>(
		&mut self,
		world: &World,
		entity: Entity,
		mut init: F,
	) -> ComponentPairMut<'_, T>
	where
		F: FnMut() -> T,
	{
		assert!(world.is_alive(entity));

		// Try to find and replace the existing entity in the map.
		if let Some(slot) = self.map.find(entity.hash_index(), |(candidate, _)| {
			// See reasoning in "insert".
			candidate.index == entity.index
		}) {
			let (found, comp) = unsafe { slot.as_mut() };
			if entity.gen == found.gen {
				ComponentPairMut { entity, comp }
			} else {
				let _ = (found, comp); // Helps prevent stupid mistakes.
				unsafe { slot.write((entity, init())) }

				let (_, comp) = unsafe { slot.as_mut() };
				ComponentPairMut { entity, comp }
			}
		} else {
			let (_, comp) = unsafe { self.force_insert(world, entity, init()).as_mut() };
			ComponentPairMut { entity, comp }
		}
	}

	pub fn try_get_mut(
		&mut self,
		world: &World,
		entity: Entity,
	) -> Option<ComponentPairMut<'_, T>> {
		if world.is_alive(entity) {
			let bucket = self.map.find(entity.hash_index(), |(candidate, _)| {
				candidate.index == entity.index
			})?;
			let (found, component) = unsafe { bucket.as_mut() };

			if entity.gen != found.gen {
				// Ensure that these references are dead before we logically move them to avoid
				// stupid mistakes (shouldn't affect code gen/soundness).
				let _ = (found, component);

				// We know that this entity isn't from the latest generation so we can safely free it.
				drop(unsafe { self.map.remove(bucket) });
				return None;
			}

			// "comp" is bounded by the function signature. No bad references here!
			Some(ComponentPairMut {
				entity,
				comp: component,
			})
		} else {
			None
		}
	}

	pub fn get_mut(&mut self, world: &World, entity: Entity) -> ComponentPairMut<'_, T> {
		self.try_get_mut(world, entity).unwrap()
	}

	pub fn remove(&mut self, entity: Entity) -> Option<T> {
		self.map
			.remove_entry(entity.hash_index(), |(candidate, _)| {
				// This reduced equality check will either match the desired entity and thus remove it
				// or it will match a dead entity, which it should remove anyways.
				candidate.index == entity.index
			})
			.and_then(|(found, comp)| {
				if entity.gen == found.gen {
					Some(comp)
				} else {
					None
				}
			})
	}

	pub fn iter<'a>(&'a self, world: &'a World) -> StorageIterEntryRef<'a, T> {
		StorageIterEntryRef::new(self, world)
	}

	pub fn iter_mut<'a>(&'a mut self, world: &'a World) -> StorageIterEntryMut<'a, T> {
		StorageIterEntryMut::new(self, world)
	}

	pub fn clean(&mut self, world: &World) {
		for _ in self.iter_mut(world) {}
	}
}

pub struct StorageIterEntryRef<'a, T> {
	storage: &'a MapStorage<T>,
	world: &'a World,
	iter: RawIter<(Entity, T)>,
}

impl<'a, T> StorageIterEntryRef<'a, T> {
	pub fn new(storage: &'a MapStorage<T>, world: &'a World) -> Self {
		Self {
			storage,
			world,
			iter: unsafe { storage.map.iter() },
		}
	}
}

impl<T> Clone for StorageIterEntryRef<'_, T> {
	fn clone(&self) -> Self {
		Self {
			storage: self.storage,
			world: self.world,
			iter: self.iter.clone(),
		}
	}
}

impl<'a, T> Iterator for StorageIterEntryRef<'a, T> {
	type Item = ComponentPair<'a, T>;

	fn next(&mut self) -> Option<Self::Item> {
		for bucket in &mut self.iter {
			let (entity, comp) = unsafe { bucket.as_ref() };
			if self.world.is_alive(*entity) {
				return Some(ComponentPair {
					entity: *entity,
					comp,
				});
			}
		}
		None
	}
}

pub struct StorageIterEntryMut<'a, T> {
	world: &'a World,
	map: &'a mut RawTable<(Entity, T)>,
	iter: RawIter<(Entity, T)>,
}

impl<'a, T> StorageIterEntryMut<'a, T> {
	pub fn new(storage: &'a mut MapStorage<T>, world: &'a World) -> Self {
		let iter = unsafe { storage.map.iter() };
		Self {
			world,
			map: &mut storage.map,
			iter,
		}
	}
}

impl<'a, T> Iterator for StorageIterEntryMut<'a, T> {
	type Item = ComponentPairMut<'a, T>;

	fn next(&mut self) -> Option<Self::Item> {
		for bucket in &mut self.iter {
			let (entity, comp) = unsafe { bucket.as_mut() };
			if self.world.is_alive(*entity) {
				return Some(ComponentPairMut {
					entity: *entity,
					comp,
				});
			} else {
				let _ = (entity, comp);
				// `map.iter()` doesn't bound the lifetime of the iterator so this "double borrow"
				// is legal.
				drop(unsafe { self.map.remove(bucket) });
			}
		}
		None
	}
}
