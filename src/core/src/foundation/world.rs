use std::cell::{Ref, RefCell};
use std::collections::VecDeque;
use std::ops::Deref;

pub struct WorldAccessor {
	world: RefCell<hecs::World>,
	actions: RefCell<VecDeque<()>>,
}

impl WorldAccessor {
	pub fn world(&self) -> WorldBorrow {
		WorldBorrow {
			wrapper: self,
			world: self.world.borrow(),
		}
	}

	pub fn insert(
		&self,
		entity: hecs::Entity,
		bundle: impl hecs::DynamicBundle,
	) -> Result<(), hecs::NoSuchEntity> {
		if let Ok(mut world) = self.world.try_borrow_mut() {
			world.insert(entity, bundle)
		} else {
			todo!()
		}
	}
}

pub struct WorldBorrow<'a> {
	wrapper: &'a WorldAccessor,
	world: Ref<'a, hecs::World>,
}

impl Deref for WorldBorrow<'_> {
	type Target = hecs::World;

	fn deref(&self) -> &Self::Target {
		&*self.world
	}
}
