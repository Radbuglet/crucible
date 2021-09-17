use hecs::{DynamicBundle, Entity, World};
use std::cell::{Ref, RefCell};
use std::ops::Deref;

// TODO: Multithreading, optimize action queue (e.g. through bumpalo or a bespoke unsized queue)
#[derive(Default)]
pub struct WorldAccessor {
	world: RefCell<World>,
	queue: RefCell<Vec<Action>>,
}

impl WorldAccessor {
	pub fn new(world: World) -> Self {
		Self {
			world: RefCell::new(world),
			queue: Default::default(),
		}
	}

	pub fn query(&self) -> WorldBorrow {
		WorldBorrow {
			wrapper: self,
			world: Some(self.world.borrow()),
		}
	}

	pub fn spawn(&self, bundle: impl 'static + DynamicBundle) -> Entity {
		if let Ok(mut world) = self.world.try_borrow_mut() {
			world.spawn(bundle)
		} else {
			let entity = self.world.borrow().reserve_entity();
			self.insert(entity, bundle); // TODO: Don't queue empty bundles.
			entity
		}
	}

	pub fn insert(&self, entity: Entity, bundle: impl 'static + DynamicBundle) {
		// TODO: Figure out insertion failure situation
		if let Ok(mut world) = self.world.try_borrow_mut() {
			let _ = world.insert(entity, bundle);
		} else {
			self.queue.borrow_mut().push(Action::Insert {
				entity,
				bundle: Box::new(Some(bundle)) as Box<dyn AnyBundle>,
			})
		}
	}
}

pub struct WorldBorrow<'a> {
	wrapper: &'a WorldAccessor,
	world: Option<Ref<'a, World>>,
}

impl Deref for WorldBorrow<'_> {
	type Target = World;

	fn deref(&self) -> &Self::Target {
		&*self.world.as_ref().unwrap()
	}
}

impl Drop for WorldBorrow<'_> {
	fn drop(&mut self) {
		drop(self.world.take());
		if let Ok(mut world) = self.wrapper.world.try_borrow_mut() {
			for action in self.wrapper.queue.borrow_mut().drain(..) {
				action.apply(&mut *world);
			}
		}
	}
}

enum Action {
	Insert {
		entity: Entity,
		bundle: Box<dyn AnyBundle>,
	},
}

impl Action {
	pub fn apply(self, world: &mut World) {
		match self {
			Action::Insert { entity, mut bundle } => bundle.insert(entity, world),
		}
	}
}

/// An object-safe wrapper around [DynamicBundle].
trait AnyBundle {
	fn spawn(&mut self, world: &mut World) -> Entity;
	fn insert(&mut self, entity: Entity, world: &mut World);
}

impl<T: DynamicBundle> AnyBundle for Option<T> {
	fn spawn(&mut self, world: &mut World) -> Entity {
		world.spawn(self.take().unwrap())
	}

	fn insert(&mut self, entity: Entity, world: &mut World) {
		let _ = world.insert(entity, self.take().unwrap());
	}
}
