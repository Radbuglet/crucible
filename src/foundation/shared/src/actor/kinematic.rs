use crate::{
	actor::collider::Collider,
	math::{kinematic::update_kinematic, Aabb3, Axis3, BlockFace, EntityVec, Sign, VecCompExt},
	voxel::{
		collision::{
			cast_volume, filter_all_colliders, move_rigid_body, MaterialColliderDescriptor,
			COLLISION_TOLERANCE,
		},
		data::{BlockMaterialRegistry, ChunkVoxelData, WorldVoxelData},
	},
};
use bort::{cx, Cx, HasGlobalManagedTag};
use crucible_util::mem::c_enum::{CEnum, CEnumMap};

use super::spatial::Spatial;

// === Context === //

type CxSideOcclusion<'a> = Cx<&'a ChunkVoxelData, &'a MaterialColliderDescriptor>;
type CxApplyPhysics<'a> = Cx<&'a ChunkVoxelData, &'a MaterialColliderDescriptor, &'a mut Collider>;

// === Components === //

#[derive(Debug)]
pub struct KinematicObject {
	// 6 booleans v.s. a 1 byte bitset have no impact on the size of this struct because of the
	// alignment requirements of an f64.
	pub collision_mask: CEnumMap<BlockFace, bool>,
	pub velocity: EntityVec,
	pub acceleration: EntityVec,
	pub friction: EntityVec,
}

impl HasGlobalManagedTag for KinematicObject {
	type Component = Self;
}

impl KinematicObject {
	pub fn new(friction: EntityVec) -> Self {
		Self {
			collision_mask: CEnumMap::default(),
			velocity: EntityVec::ZERO,
			acceleration: EntityVec::ZERO,
			friction,
		}
	}

	#[clippy::dangerous(
		direct_kinematic_updating,
		reason = "physics updating should be deferred to its dedicated system"
	)]
	pub fn apply_physics(
		&mut self,
		cx: CxApplyPhysics<'_>,
		world: &WorldVoxelData,
		registry: &BlockMaterialRegistry,
		spatial: &mut Spatial,
		collider: &Collider,
		delta: f64,
	) {
		// Clip velocities and accelerations into obstructed faces
		self.update_face_touching_mask(cx!(cx), world, registry, collider);

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
		let (delta_pos, velocity) =
			update_kinematic(self.velocity, self.acceleration, self.friction, delta);

		self.velocity = velocity;

		// Apply desired position change
		let pos_delta = {
			let aabb = collider.aabb();
			let new_origin = move_rigid_body(
				cx!(cx),
				world,
				registry,
				aabb,
				delta_pos,
				filter_all_colliders(),
			);

			new_origin - aabb.origin
		};

		spatial.pos += pos_delta;
	}

	// === Collisions === //

	fn is_face_touching_now_inner(
		cx: CxSideOcclusion<'_>,
		world: &WorldVoxelData,
		registry: &BlockMaterialRegistry,
		aabb: Aabb3<EntityVec>,
		face: BlockFace,
	) -> bool {
		let additional_margin = COLLISION_TOLERANCE;

		cast_volume(
			cx!(cx),
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
		cx: CxSideOcclusion<'_>,
		world: &WorldVoxelData,
		registry: &BlockMaterialRegistry,
		collider: &Collider,
		face: BlockFace,
	) -> bool {
		Self::is_face_touching_now_inner(cx!(cx), world, registry, collider.aabb(), face)
	}

	pub fn update_face_touching_mask(
		&mut self,
		cx: CxSideOcclusion<'_>,
		world: &WorldVoxelData,
		registry: &BlockMaterialRegistry,
		collider: &Collider,
	) {
		for (face, state) in self.collision_mask.iter_mut() {
			*state =
				Self::is_face_touching_now_inner(cx!(cx), world, registry, collider.aabb(), face);
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
