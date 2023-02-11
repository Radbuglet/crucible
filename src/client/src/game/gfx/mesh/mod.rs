use crucible_common::voxel::math::{BlockFace, Sign, Vec3Ext};
use crucible_util::mem::c_enum::CEnum;
use typed_glam::glam::{Vec2, Vec3};

use crate::engine::gfx::geometry;

#[derive(Debug, Clone)]
pub struct QuadMeshLayer<M> {
	pub quads: Vec<Quad<M>>,
}

#[derive(Debug, Copy, Clone)]
pub struct Quad<M> {
	pub face: BlockFace,
	pub origin: Vec3,
	pub size: Vec2,
	pub material: M,
}

impl<M> QuadMeshLayer<M> {
	pub fn push_cube_faces_hetero<I>(&mut self, origin: Vec3, volume: Vec3, faces: I)
	where
		I: IntoIterator<Item = (BlockFace, M)>,
	{
		for (face, material) in faces {
			let (axis, sign) = face.decompose();

			let origin = if sign == Sign::Positive {
				origin + volume.comp(axis)
			} else {
				origin
			};

			let size = geometry::face_size_given_volume(volume, axis);

			self.quads.push(Quad {
				face,
				origin,
				size,
				material,
			});
		}
	}

	pub fn push_cube_faces<I>(&mut self, origin: Vec3, volume: Vec3, material: M, faces: I)
	where
		M: Copy,
		I: IntoIterator<Item = BlockFace>,
	{
		self.push_cube_faces_hetero(
			origin,
			volume,
			faces.into_iter().map(|face| (face, material)),
		);
	}

	pub fn push_cube(&mut self, origin: Vec3, volume: Vec3, material: M)
	where
		M: Copy,
	{
		self.push_cube_faces(origin, volume, material, BlockFace::variants());
	}
}
