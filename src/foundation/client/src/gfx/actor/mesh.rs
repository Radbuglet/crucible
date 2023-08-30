use crucible_foundation_shared::{math::Color3, voxel::mesh::QuadMeshLayer};
use typed_glam::glam::UVec2;

#[derive(Debug)]
pub struct ActorRenderer {}

impl ActorRenderer {
	pub fn push_model(&mut self, model: &QuadMeshLayer<(UVec2, Color3)>) {}

	pub fn push_model_instance(&mut self, affine: ()) {}

	pub fn render() {}
}
