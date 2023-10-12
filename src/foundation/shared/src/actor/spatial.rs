use bort::HasGlobalManagedTag;

use crate::math::EntityVec;

#[derive(Debug, Clone, Default)]
pub struct Spatial {
	pub pos: EntityVec,
}

impl HasGlobalManagedTag for Spatial {
	type Component = Self;
}
