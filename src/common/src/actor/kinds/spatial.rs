use bort::storage;
use typed_glam::traits::NumericVector;

use crate::{
	actor::manager::{ActorManager, Tag},
	world::{
		coord::move_rigid_body,
		data::VoxelWorldData,
		math::{Aabb3, Angle3D, Angle3DExt, EntityVec},
	},
};

#[derive(Debug)]
pub struct Spatial {
	pub pos: EntityVec,
	pub rot: Angle3D,
}

impl Spatial {
	pub fn facing(&self) -> EntityVec {
		self.rot.forward().as_dvec3().cast()
	}
}

#[derive(Debug)]
pub struct KinematicSpatial {
	pub vel: EntityVec,
	pub collider: Aabb3<EntityVec>,
}

#[non_exhaustive]
pub struct KinematicUpdateTag;

impl Tag for KinematicUpdateTag {}

pub fn update_kinematic_spatials(actors: &ActorManager, world: &VoxelWorldData) {
	let spatials = storage::<Spatial>();
	let kinematics = storage::<KinematicSpatial>();

	for player in actors.tagged::<KinematicUpdateTag>() {
		let spatial = &mut *spatials.get_mut(player);
		let kinematic = &mut *kinematics.get_mut(player);

		spatial.pos = move_rigid_body(
			world,
			kinematic.collider.translated(spatial.pos),
			kinematic.vel,
		) - kinematic.collider.origin;
	}
}
