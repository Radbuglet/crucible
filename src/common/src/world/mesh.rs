use crucible_util::mem::c_enum::CEnum;
use derive_where::derive_where;
use typed_glam::glam::Vec3;

use super::math::{AaQuad, Aabb3, BlockFace};

// === Volumetric === //

#[derive(Debug, Clone)]
#[derive_where(Default)]
pub struct VolumetricMeshLayer<M> {
	pub aabbs: Vec<(Aabb3<Vec3>, M)>,
}

impl<M> VolumetricMeshLayer<M> {
	pub fn push_aabb(&mut self, aabb: Aabb3<Vec3>, material: M) {
		self.aabbs.push((aabb, material));
	}

	pub fn with_aabb(mut self, aabb: Aabb3<Vec3>, material: M) -> Self {
		self.push_aabb(aabb, material);
		self
	}

	pub fn quads(&self) -> impl Iterator<Item = StyledQuad<&M>> + '_ {
		self.aabbs
			.iter()
			.map(|(aabb, material)| {
				BlockFace::variants().map(move |face| StyledQuad {
					quad: aabb.quad(face),
					material,
				})
			})
			.flatten()
	}

	pub fn quads_cloned(&self) -> impl Iterator<Item = StyledQuad<M>> + '_
	where
		M: Clone,
	{
		self.quads().map(|v| v.cloned())
	}

	pub fn as_mesh_layer(&self) -> QuadMeshLayer<M>
	where
		M: Clone,
	{
		QuadMeshLayer::from_iter(self.quads_cloned())
	}
}

// === Quads === //

#[derive(Debug, Copy, Clone)]
pub struct StyledQuad<M> {
	pub quad: AaQuad<Vec3>,
	pub material: M,
}

impl<M> StyledQuad<M> {
	pub fn as_ref(&self) -> StyledQuad<&M> {
		StyledQuad {
			quad: self.quad,
			material: &self.material,
		}
	}
}

impl<M> StyledQuad<&'_ M> {
	pub fn cloned(&self) -> StyledQuad<M>
	where
		M: Clone,
	{
		StyledQuad {
			quad: self.quad,
			material: self.material.clone(),
		}
	}

	pub fn copied(&self) -> StyledQuad<M>
	where
		M: Copy,
	{
		StyledQuad {
			quad: self.quad,
			material: *self.material,
		}
	}
}

#[derive(Debug, Clone)]
#[derive_where(Default)]
pub struct QuadMeshLayer<M> {
	pub quads: Vec<StyledQuad<M>>,
}

impl<M> QuadMeshLayer<M> {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn from_iter(iter: impl IntoIterator<Item = StyledQuad<M>>) -> Self {
		Self {
			quads: Vec::from_iter(iter),
		}
	}

	pub fn push_cube_faces_hetero<I>(&mut self, aabb: Aabb3<Vec3>, faces: I)
	where
		I: IntoIterator<Item = (BlockFace, M)>,
	{
		for (face, material) in faces {
			self.quads.push(StyledQuad {
				quad: aabb.quad(face),
				material,
			});
		}
	}

	pub fn push_cube_faces<I>(&mut self, aabb: Aabb3<Vec3>, material: M, faces: I)
	where
		M: Copy,
		I: IntoIterator<Item = BlockFace>,
	{
		self.push_cube_faces_hetero(aabb, faces.into_iter().map(|face| (face, material)));
	}

	pub fn push_cube(&mut self, aabb: Aabb3<Vec3>, material: M)
	where
		M: Copy,
	{
		self.push_cube_faces(aabb, material, BlockFace::variants());
	}

	pub fn with_cube(mut self, aabb: Aabb3<Vec3>, material: M) -> Self
	where
		M: Copy,
	{
		self.push_cube(aabb, material);
		self
	}

	pub fn with_quads(mut self, iter: impl IntoIterator<Item = StyledQuad<M>>) -> Self {
		self.extend(iter);
		self
	}
}

impl<M> Extend<StyledQuad<M>> for QuadMeshLayer<M> {
	fn extend<T: IntoIterator<Item = StyledQuad<M>>>(&mut self, iter: T) {
		self.quads.extend(iter);
	}
}
