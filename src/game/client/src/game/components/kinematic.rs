use bort::{
	saddle::{cx, BortComponents},
	CompMut, HasGlobalManagedTag,
};
use crucible_foundation_shared::{
	actor::spatial::{Spatial, SpatialMutateCx, SpatialTracker},
	material::MaterialRegistry,
	math::{kinematic::update_kinematic, Aabb3, Axis3, BlockFace, EntityVec, Sign, VecCompExt},
	voxel::{
		collision::{
			cast_volume, filter_all_colliders, move_rigid_body, ColliderCheckCx,
			COLLISION_TOLERANCE,
		},
		data::WorldVoxelData,
	},
};
use crucible_util::mem::c_enum::{CEnum, CEnumMap};

// === Context === //

cx! {
	pub trait CxSideOcclusion(BortComponents): ColliderCheckCx;
	pub trait CxApplyPhysics(BortComponents): CxSideOcclusion, SpatialMutateCx;
}

// === Components === //

#[derive(Debug)]
pub struct KinematicSpatial {
	// 6 booleans v.s. a 1 byte bitset have no impact on the size of this struct because of the
	// alignment requirements of an f64.
	pub collision_mask: CEnumMap<BlockFace, bool>,
	pub velocity: EntityVec,
	pub acceleration: EntityVec,
	pub friction: EntityVec,
}

impl HasGlobalManagedTag for KinematicSpatial {
	type Component = Self;
}

impl KinematicSpatial {
	pub fn new(friction: EntityVec) -> Self {
		Self {
			collision_mask: CEnumMap::default(),
			velocity: EntityVec::ZERO,
			acceleration: EntityVec::ZERO,
			friction,
		}
	}

	pub fn apply_physics(
		&mut self,
		cx: &impl CxApplyPhysics,
		world: &WorldVoxelData,
		registry: &MaterialRegistry,
		spatial_mgr: &mut SpatialTracker,
		spatial: &mut CompMut<Spatial>,
		delta: f64,
	) {
		// Clip velocities and accelerations into obstructed faces
		self.update_face_touching_mask(cx, world, registry, spatial);

		for axis in Axis3::variants() {
			let clip_comp = |comp: &mut f64| {
				let sign = Sign::of(*comp).unwrap_or(Sign::Positive);
				let face = BlockFace::compose(axis, sign);

				if self.collision_mask[face] {
					*comp = 0.0;
				}
			};

			// N.B. we do these separately because a player could be accelerating
			// in the direction opposite to which they are moving.
			clip_comp(self.velocity.comp_mut(axis));
			clip_comp(self.acceleration.comp_mut(axis));
		}

		// Update velocity and position
		let aabb = spatial.aabb();
		let (delta_pos, velocity) =
			update_kinematic(self.velocity, self.acceleration, self.friction, delta);

		self.velocity = velocity;
		let new_origin =
			move_rigid_body(cx, world, registry, aabb, delta_pos, filter_all_colliders());

		spatial_mgr.update(cx, spatial, aabb.with_origin(new_origin));
	}

	// === Collisions === //

	fn is_face_touching_now_inner(
		cx: &impl CxSideOcclusion,
		world: &WorldVoxelData,
		registry: &MaterialRegistry,
		aabb: Aabb3<EntityVec>,
		face: BlockFace,
	) -> bool {
		let additional_margin = COLLISION_TOLERANCE;

		cast_volume(
			cx,
			world,
			registry,
			aabb.quad(face),
			additional_margin,
			COLLISION_TOLERANCE,
			filter_all_colliders(),
		) < additional_margin / 2.0
	}

	pub fn is_face_touching_now(
		&self,
		cx: &impl CxSideOcclusion,
		world: &WorldVoxelData,
		registry: &MaterialRegistry,
		spatial: &Spatial,
		face: BlockFace,
	) -> bool {
		Self::is_face_touching_now_inner(cx, world, registry, spatial.aabb(), face)
	}

	pub fn update_face_touching_mask(
		&mut self,
		cx: &impl CxSideOcclusion,
		world: &WorldVoxelData,
		registry: &MaterialRegistry,
		spatial: &Spatial,
	) {
		for (face, state) in self.collision_mask.iter_mut() {
			*state = Self::is_face_touching_now_inner(cx, world, registry, spatial.aabb(), face);
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
