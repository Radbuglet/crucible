use bort::storage;
use crucible_util::mem::c_enum::{CEnum, CEnumMap};
use typed_glam::traits::NumericVector;

use crate::{
	actor::{
		kinematic::update_kinematic,
		manager::{ActorManager, Tag},
	},
	world::{
		collision::{cast_volume, move_rigid_body, COLLISION_TOLERANCE},
		data::VoxelWorldData,
		math::{Aabb3, Angle3D, Angle3DExt, Axis3, BlockFace, EntityVec, Sign, Vec3Ext},
	},
};

// === Spatial === //

#[derive(Debug)]
pub struct Spatial {
	pub position: EntityVec,
	pub rotation: Angle3D,
}

impl Spatial {
	pub fn facing(&self) -> EntityVec {
		self.rotation.forward().as_dvec3().cast()
	}
}

// === KinematicSpatial === //

#[derive(Debug)]
pub struct KinematicSpatial {
	// 6 booleans v.s. a 1 byte bitset have no impact on the size of this struct because of the
	// alignment requirements of an f64.
	pub collision_mask: CEnumMap<BlockFace, bool>,
	pub velocity: EntityVec,
	pub acceleration: EntityVec,
	pub friction: EntityVec,
	pub collider: Aabb3<EntityVec>,
}

impl KinematicSpatial {
	pub fn new(collider: Aabb3<EntityVec>, friction: EntityVec) -> Self {
		Self {
			collision_mask: CEnumMap::default(),
			velocity: EntityVec::ZERO,
			acceleration: EntityVec::ZERO,
			friction,
			collider,
		}
	}

	// === Collisions === //

	pub fn current_collider(&self, spatial: &Spatial) -> Aabb3<EntityVec> {
		self.collider.translated(spatial.position)
	}

	fn is_face_touching_now_inner(
		world: &VoxelWorldData,
		aabb: Aabb3<EntityVec>,
		face: BlockFace,
	) -> bool {
		let additional_margin = COLLISION_TOLERANCE;

		cast_volume(
			world,
			aabb.quad(face),
			additional_margin,
			COLLISION_TOLERANCE,
		) < additional_margin / 2.0
	}

	pub fn is_face_touching_now(
		&self,
		world: &VoxelWorldData,
		spatial: &Spatial,
		face: BlockFace,
	) -> bool {
		Self::is_face_touching_now_inner(world, self.current_collider(spatial), face)
	}

	pub fn update_face_touching_mask(&mut self, world: &VoxelWorldData, spatial: &Spatial) {
		let aabb = self.current_collider(spatial);

		for (face, state) in self.collision_mask.iter_mut() {
			*state = Self::is_face_touching_now_inner(world, aabb, face);
		}
	}

	pub fn was_face_touching(&self, face: BlockFace) -> bool {
		self.collision_mask[face]
	}

	// === Physics === //

	pub fn apply_impulse(&mut self, impulse: EntityVec) {
		self.velocity += impulse;
	}

	pub fn apply_acceleration(&mut self, acceleration: EntityVec) {
		self.acceleration += acceleration;
	}
}

#[non_exhaustive]
pub struct KinematicUpdateTag;

impl Tag for KinematicUpdateTag {}

pub fn update_kinematic_spatials(actors: &ActorManager, world: &VoxelWorldData, time: f64) {
	let spatials = storage::<Spatial>();
	let kinematics = storage::<KinematicSpatial>();

	for player in actors.tagged::<KinematicUpdateTag>() {
		let spatial = &mut *spatials.get_mut(player);
		let kinematic = &mut *kinematics.get_mut(player);

		// Clip velocities and accelerations into obstructed faces
		kinematic.update_face_touching_mask(world, spatial);

		for axis in Axis3::variants() {
			// N.B. we do these separetly because a player could be accelerating in
			// the opposite direction than which they are moving.

			let clip_comp = |comp: &mut f64| {
				let sign = Sign::of(*comp).unwrap_or(Sign::Positive);
				let face = BlockFace::compose(axis, sign);

				if kinematic.collision_mask[face] {
					*comp = 0.0;
				}
			};

			clip_comp(kinematic.velocity.comp_mut(axis));
			clip_comp(kinematic.acceleration.comp_mut(axis));
		}

		// Update velocity and position
		let aabb = kinematic.current_collider(spatial);
		let (delta_pos, velocity) = update_kinematic(
			kinematic.velocity,
			kinematic.acceleration,
			kinematic.friction,
			time,
		);

		kinematic.velocity = velocity;

		let new_pos = move_rigid_body(world, aabb, delta_pos) - kinematic.collider.origin;
		spatial.position = new_pos;
	}
}
