use crucible_util::mem::c_enum::CEnum;
use derive_where::derive_where;
use typed_glam::glam::Vec3;

use crate::math::{AaQuad, Aabb3, BlockFace};

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

	pub fn quads(&self) -> impl Iterator<Item = (AaQuad<Vec3>, &M)> + '_ {
		self.aabbs.iter().flat_map(|(aabb, material)| {
			BlockFace::variants().map(move |face| (aabb.quad(face), material))
		})
	}

	pub fn quads_cloned(&self) -> impl Iterator<Item = (AaQuad<Vec3>, M)> + '_
	where
		M: Clone,
	{
		self.quads().map(|(quad, mat)| (quad, mat.clone()))
	}

	pub fn as_mesh_layer(&self) -> QuadMeshLayer<M>
	where
		M: Clone,
	{
		QuadMeshLayer::from_iter(self.quads_cloned())
	}

	pub fn iter(&self) -> impl Iterator<Item = (Aabb3<Vec3>, &M)> + '_ {
		self.aabbs.iter().map(|(aabb, material)| (*aabb, material))
	}

	pub fn iter_cloned(&self) -> impl Iterator<Item = (Aabb3<Vec3>, M)> + '_
	where
		M: Clone,
	{
		self.aabbs.iter().cloned()
	}
}

// === Quads === //

#[derive(Debug, Clone)]
#[derive_where(Default)]
pub struct QuadMeshLayer<M> {
	pub quads: Vec<(AaQuad<Vec3>, M)>,
}

impl<M> QuadMeshLayer<M> {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn push_cube_faces_hetero<I>(&mut self, aabb: Aabb3<Vec3>, faces: I)
	where
		I: IntoIterator<Item = (BlockFace, M)>,
	{
		for (face, material) in faces {
			self.quads.push((aabb.quad(face), material));
		}
	}

	pub fn push_cube_faces<I>(&mut self, aabb: Aabb3<Vec3>, material: M, faces: I)
	where
		M: Clone,
		I: IntoIterator<Item = BlockFace>,
	{
		self.push_cube_faces_hetero(aabb, faces.into_iter().map(|face| (face, material.clone())));
	}

	pub fn push_cube(&mut self, aabb: Aabb3<Vec3>, material: M)
	where
		M: Clone,
	{
		self.push_cube_faces(aabb, material, BlockFace::variants());
	}

	pub fn with_cube(mut self, aabb: Aabb3<Vec3>, material: M) -> Self
	where
		M: Clone,
	{
		self.push_cube(aabb, material);
		self
	}

	pub fn with_quads(mut self, iter: impl IntoIterator<Item = (AaQuad<Vec3>, M)>) -> Self {
		self.extend(iter);
		self
	}

	pub fn iter(&self) -> impl Iterator<Item = (AaQuad<Vec3>, &M)> + '_ {
		self.quads.iter().map(|(quad, mat)| (*quad, mat))
	}

	pub fn iter_cloned(&self) -> impl Iterator<Item = (AaQuad<Vec3>, M)> + '_
	where
		M: Clone,
	{
		self.quads.iter().cloned()
	}
}

impl<M> FromIterator<(AaQuad<Vec3>, M)> for QuadMeshLayer<M> {
	fn from_iter<T: IntoIterator<Item = (AaQuad<Vec3>, M)>>(iter: T) -> Self {
		Self {
			quads: Vec::from_iter(iter),
		}
	}
}

impl<M> Extend<(AaQuad<Vec3>, M)> for QuadMeshLayer<M> {
	fn extend<T: IntoIterator<Item = (AaQuad<Vec3>, M)>>(&mut self, iter: T) {
		self.quads.extend(iter);
	}
}
