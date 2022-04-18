//! A comically inefficient proof-of-concept implementation of the Geode ECS. This is basically just a
//! sandbox to try out API designs and to standardize API behavior with a simple and probably-correct
//! implementation.

#![deny(unsafe_code)]

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Default)]
pub struct World(RefCell<WorldInner>);

#[derive(Debug, Default)]
struct WorldInner {
	id_gen: u64,
	alive_now: HashSet<Entity>,
	create_later: Vec<Entity>,
	delete_later: Vec<Entity>,
}

impl World {
	pub fn new() -> Self {
		Default::default()
	}

	pub fn spawn_now(&mut self) -> Entity {
		let world = self.0.get_mut();
		world.id_gen += 1;
		let id = Entity(world.id_gen);
		world.alive_now.insert(id);
		id
	}

	pub fn spawn_later(&self) -> Entity {
		let mut world = self.0.borrow_mut();
		world.id_gen += 1;
		let id = Entity(world.id_gen);
		world.alive_now.insert(id);
		id
	}

	pub fn despawn_now(&mut self, id: Entity) {
		let world = self.0.get_mut();
		let removed = world.alive_now.remove(&id);
		assert!(removed);
	}

	pub fn despawn_later(&self, id: Entity) {
		let mut world = self.0.borrow_mut();
		world.delete_later.push(id);
	}

	pub fn is_alive_now(&self, id: Entity) -> bool {
		self.0.borrow().alive_now.contains(&id)
	}

	pub fn is_alive_future(&self, id: Entity) -> bool {
		self.0.borrow().create_later.binary_search(&id).is_ok()
	}

	pub fn is_alive_or_future(&self, id: Entity) -> bool {
		self.is_alive_now(id) || self.is_alive_future(id)
	}

	pub fn flush(&mut self) {
		let world = self.0.get_mut();
		world.alive_now.extend(world.create_later.drain(..));
		for deleted in world.delete_later.drain(..) {
			world.alive_now.remove(&deleted);
		}
	}
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct Entity(u64);

#[derive(Debug)]
pub struct MapStorage<T> {
	values: HashMap<Entity, T>,
}

impl<T> Default for MapStorage<T> {
	fn default() -> Self {
		Self {
			values: HashMap::new(),
		}
	}
}

impl<T> MapStorage<T> {
	pub fn new() -> Self {
		Default::default()
	}

	pub fn insert(&mut self, _world: &World, key: Entity, value: T) -> Option<T> {
		self.values.insert(key, value)
	}

	pub fn remove(&mut self, id: Entity) -> Option<T> {
		self.values.remove(&id)
	}

	pub fn get(&self, id: Entity) -> Option<&T> {
		self.values.get(&id)
	}

	pub fn get_mut(&mut self, id: Entity) -> Option<&mut T> {
		self.values.get_mut(&id)
	}

	pub fn clear(&mut self) {
		self.values.clear();
	}

	pub fn iter(&self) -> impl Iterator<Item = (Entity, &T)> {
		self.values.iter().map(|(id, val)| (*id, val))
	}

	pub fn iter_mut(&mut self) -> impl Iterator<Item = (Entity, &mut T)> {
		self.values.iter_mut().map(|(id, val)| (*id, val))
	}

	pub fn flush(&mut self, world: &World) {
		self.values.retain(|k, _| world.is_alive_or_future(*k))
	}
}

// The illusion of free choice.
pub use MapStorage as ArchStorage;

// TODO: Prototype queries

// pub trait Query<P> {
// 	type Entry;
// 	type Iterator: Iterator<Item = Self::Entry>;
//
// 	fn query(target: P) -> Self::Iterator;
// }
