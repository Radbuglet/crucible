use crucible_common::game::material::MaterialStateBase;
use geode::{bundle, BuildableArchetypeBundle};
use typed_glam::glam::UVec2;

bundle! {
	#[derive(Debug)]
	pub struct InvisibleBlockDescriptorBundle {
		pub base: MaterialStateBase,
	}

	#[derive(Debug)]
	pub struct BasicMaterialDescriptorBundle {
		pub base: MaterialStateBase,
		pub visual: MaterialStateVisualBlock,
	}
}

impl BuildableArchetypeBundle for InvisibleBlockDescriptorBundle {}
impl BuildableArchetypeBundle for BasicMaterialDescriptorBundle {}

#[derive(Debug)]
pub struct MaterialStateVisualBlock {
	pub atlas_tile: UVec2,
}
