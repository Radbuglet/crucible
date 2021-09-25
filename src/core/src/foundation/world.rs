//! A work-in-progress ECS.
//! TODO: Stop leaking memory, make *much* more efficient, determine change semantics

use hibitset::{BitSet, BitSetLike};
use std::collections::HashMap;
use std::ops::{Index, IndexMut};

#[derive(Default)]
pub struct World {
	reserved: BitSet,
	counter: u32,
	generations: Vec<u32>,
}

impl World {
	pub fn new() -> Self {
		Default::default()
	}

	pub fn spawn(&mut self) -> Entity {
		// Reserve an index and increment the generation
		let idx = match (!(&self.reserved)).iter().next() {
			Some(idx) if (idx as usize) < self.generations.len() => {
				self.generations[idx as usize] += 1;
				idx
			}
			_ => {
				let alloc = self.counter;
				self.counter = self
					.counter
					.checked_add(1)
					.expect("Failed to allocate a new entity!");
				self.generations.push(0);
				alloc
			}
		};

		// Mark slot as reserved and create
		self.reserved.add(idx);
		Entity {
			idx,
			gen: self.generations[idx as usize],
		}
	}

	pub fn despawn(&mut self, entity: Entity) -> bool {
		self.reserved.remove(entity.idx)
	}

	pub fn is_alive(&self, entity: Entity) -> bool {
		self.generations[entity.idx as usize] == entity.gen
	}
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct Entity {
	idx: u32,
	gen: u32,
}

pub struct MapStorage<T> {
	map: HashMap<Entity, T>,
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

	pub fn insert(&mut self, entity: Entity, value: T) -> Option<T> {
		self.map.insert(entity, value)
	}

	pub fn get(&self, entity: Entity) -> Option<&T> {
		self.map.get(&entity)
	}

	pub fn get_mut(&mut self, entity: Entity) -> Option<&mut T> {
		self.map.get_mut(&entity)
	}

	pub fn remove(&mut self, id: Entity) -> Option<T> {
		self.map.remove(&id)
	}

	pub fn iter<'a>(&'a self, world: &'a World) -> impl Iterator<Item = (Entity, &T)> + 'a {
		self.map.iter().filter_map(move |(k, v)| {
			if world.is_alive(*k) {
				Some((*k, v))
			} else {
				None
			}
		})
	}

	pub fn iter_mut<'a>(
		&'a mut self,
		world: &'a World,
	) -> impl Iterator<Item = (Entity, &mut T)> + 'a {
		self.map.iter_mut().filter_map(move |(k, v)| {
			if world.is_alive(*k) {
				Some((*k, v))
			} else {
				None
			}
		})
	}
}

impl<T> Index<Entity> for MapStorage<T> {
	type Output = T;

	fn index(&self, index: Entity) -> &Self::Output {
		self.get(index).unwrap()
	}
}

impl<T> IndexMut<Entity> for MapStorage<T> {
	fn index_mut(&mut self, index: Entity) -> &mut Self::Output {
		self.get_mut(index).unwrap()
	}
}
