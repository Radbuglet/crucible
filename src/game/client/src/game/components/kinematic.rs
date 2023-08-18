use bort::{
	saddle::{cx, BortComponents},
	HasGlobalManagedTag,
};
use crucible_foundation_shared::{
	actor::spatial::Spatial,
	material::MaterialRegistry,
	math::{Aabb3, BlockFace, EntityVec},
	voxel::{
		collision::{self, cast_volume, filter_all_colliders, COLLISION_TOLERANCE},
		data::WorldVoxelData,
	},
};
use crucible_util::mem::c_enum::CEnumMap;

// === Context === //

cx! {
	pub trait CxMut(BortComponents): collision::CxRef;
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

	// === Collisions === //

	fn is_face_touching_now_inner(
		cx: &impl CxMut,
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
		cx: &impl CxMut,
		world: &WorldVoxelData,
		registry: &MaterialRegistry,
		spatial: &Spatial,
		face: BlockFace,
	) -> bool {
		Self::is_face_touching_now_inner(cx, world, registry, spatial.aabb(), face)
	}

	pub fn update_face_touching_mask(
		&mut self,
		cx: &impl CxMut,
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
