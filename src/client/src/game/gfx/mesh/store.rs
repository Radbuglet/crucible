use crucible_common::voxel::math::{AaQuad, BlockFace, Sign, Vec3Ext};
use crucible_util::mem::c_enum::CEnum;
use derive_where::derive_where;
use typed_glam::glam::Vec3;

#[derive(Debug, Clone)]
#[derive_where(Default)]
pub struct QuadMeshLayer<M> {
	pub quads: Vec<StyledQuad<M>>,
}

#[derive(Debug, Copy, Clone)]
pub struct StyledQuad<M> {
	pub quad: AaQuad<Vec3>,
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
				origin + axis.unit_typed::<Vec3>() * volume.comp(axis)
			} else {
				origin
			};

			let quad = AaQuad::new_given_volume(origin, face, volume);
			self.quads.push(StyledQuad { quad, material });
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

	pub fn with_cube(mut self, origin: Vec3, volume: Vec3, material: M) -> Self
	where
		M: Copy,
	{
		self.push_cube(origin, volume, material);
		self
	}
}
