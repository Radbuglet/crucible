use crucible_common::game::material::BaseMaterialState;
use geode::{bundle, BuildableArchetypeBundle};
use typed_glam::glam::UVec2;

bundle! {
	#[derive(Debug)]
	pub struct InvisibleBlockDescriptorBundle {
		pub base: BaseMaterialState,
	}

	#[derive(Debug)]
	pub struct BasicBlockDescriptorBundle {
		pub base: BaseMaterialState,
		pub visual: BlockDescriptorVisualState,
	}
}

impl BuildableArchetypeBundle for InvisibleBlockDescriptorBundle {}
impl BuildableArchetypeBundle for BasicBlockDescriptorBundle {}

#[derive(Debug)]
pub struct BlockDescriptorVisualState {
	pub atlas_tile: UVec2,
}
