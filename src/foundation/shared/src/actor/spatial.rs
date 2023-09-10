use bort::{CompMut, EventTarget, HasGlobalManagedTag};

use crate::math::EntityVec;

#[derive(Debug, Clone)]
pub struct SpatialMoved {
	_private: (),
}

#[derive(Debug, Clone, Default)]
pub struct Spatial {
	pos: EntityVec,
}

impl HasGlobalManagedTag for Spatial {
	type Component = Self;
}

impl Spatial {
	pub fn new(pos: EntityVec) -> Self {
		Self { pos }
	}

	pub fn pos(&self) -> EntityVec {
		self.pos
	}

	pub fn set_pos(
		me: &mut CompMut<Self>,
		pos: EntityVec,
		on_moved: &mut impl EventTarget<SpatialMoved, EntityVec>,
	) {
		me.pos = pos;
		on_moved.fire(
			CompMut::owner(me).entity(),
			SpatialMoved { _private: () },
			pos,
		);
	}
}
