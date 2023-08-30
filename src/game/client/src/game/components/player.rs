use std::f64::consts::{PI, TAU};

use bort::{access_cx, storage, HasGlobalManagedTag};
use crucible_foundation_shared::{
	actor::{kinematic::KinematicSpatial, spatial::Spatial},
	material::{MaterialId, MaterialRegistry},
	math::{
		kinematic::{MC_TICKS_TO_SECS, MC_TICKS_TO_SECS_SQUARED},
		Angle3D, Angle3DExt, BlockFace, EntityAabb, EntityVec,
	},
	voxel::{
		collision::{ColliderCheckCx, RayCast},
		data::{Block, EntityVoxelPointer, VoxelDataWriteCx, WorldVoxelData},
	},
};
use crucible_util::{lang::iter::ContextualIter, use_generator};
use typed_glam::{
	glam::{DVec3, Vec3, Vec3Swizzles},
	traits::NumericVector,
};

// === Contexts === //

access_cx! {
	pub trait BlockPlacementCx: ColliderCheckCx, VoxelDataWriteCx;
}

// === Constants === //

// See: https://web.archive.org/web/20230313061131/https://www.mcpk.wiki/wiki/Jumping
pub const GRAVITY: f64 = 0.08 * MC_TICKS_TO_SECS_SQUARED;
pub const GRAVITY_VEC: EntityVec = EntityVec::from_glam(DVec3::new(0.0, -GRAVITY, 0.0));

pub const PLAYER_SPEED: f64 = 0.13 * MC_TICKS_TO_SECS_SQUARED;
pub const PLAYER_AIR_FRICTION_COEF: f64 = 0.98;
pub const PLAYER_BLOCK_FRICTION_COEF: f64 = 0.91;

pub const PLAYER_JUMP_IMPULSE: f64 = 0.43 * MC_TICKS_TO_SECS;
pub const PLAYER_JUMP_COOL_DOWN: u64 = 30;

pub const PLAYER_WIDTH: f64 = 0.8;
pub const PLAYER_HEIGHT: f64 = 1.8;
pub const PLAYER_EYE_LEVEL: f64 = 1.6;

// === Components === //

#[derive(Debug)]
pub struct LocalPlayer {
	pub facing: Angle3D,
	pub fly_mode: bool,
	pub jump_cool_down: u64,
	pub view_bob: f64,
}

impl HasGlobalManagedTag for LocalPlayer {
	type Component = Self;
}

impl LocalPlayer {
	pub fn new_aabb(pos: EntityVec) -> EntityAabb {
		EntityAabb::from_origin_size(
			pos,
			EntityVec::new(PLAYER_WIDTH, PLAYER_HEIGHT, PLAYER_WIDTH),
			EntityVec::new(0.5, 0.0, 0.5),
		)
	}

	pub fn eye_height(&self) -> f64 {
		PLAYER_EYE_LEVEL + 0.1 * self.view_bob.sin()
	}

	pub fn eye_pos(&self, spatial: &Spatial) -> EntityVec {
		spatial.aabb().at_percent(EntityVec::new(0.5, 0.0, 0.5)) + EntityVec::Y * self.eye_height()
	}

	pub fn process_movement(
		&mut self,
		kinematic: &mut KinematicSpatial,
		inputs: LocalPlayerInputs,
	) {
		// Compute heading
		let mut heading = Vec3::ZERO;

		if inputs.forward {
			heading += Vec3::Z;
		}

		if inputs.backward {
			heading -= Vec3::Z;
		}

		if inputs.left {
			heading -= Vec3::X;
		}

		if inputs.right {
			heading += Vec3::X;
		}

		// Normalize heading
		let heading = heading.normalize_or_zero();

		// Convert player-local heading to world space
		let heading = EntityVec::cast_from(
			self.facing
				.as_matrix_horizontal()
				.transform_vector3(heading),
		);

		// Accelerate in that direction
		kinematic.apply_acceleration(heading * PLAYER_SPEED);

		// Update view bob
		{
			let bob_speed = kinematic.velocity.as_glam().xz().length().sqrt() * 0.1;

			if bob_speed > 0.1 && kinematic.was_face_touching(BlockFace::NegativeY) {
				self.view_bob += bob_speed;
				self.view_bob = self.view_bob % TAU;
			} else {
				let closest_origin = if (self.view_bob - PI).abs() < PI / 2.0 {
					PI
				} else {
					0.0
				};

				let old_weight = 5.0;
				self.view_bob = (self.view_bob * old_weight + closest_origin) / (1.0 + old_weight);
			}

			if self.view_bob.is_subnormal() {
				self.view_bob = 0.0;
			}
		}

		// Handle jumps
		if !inputs.jump {
			self.jump_cool_down = 0;
		}

		if self.jump_cool_down > 0 {
			self.jump_cool_down -= 1;
		}

		if inputs.jump {
			if self.fly_mode {
				kinematic.apply_acceleration(-GRAVITY_VEC * 2.0);
			} else if self.jump_cool_down == 0 && kinematic.was_face_touching(BlockFace::NegativeY)
			{
				self.jump_cool_down = PLAYER_JUMP_COOL_DOWN;
				*kinematic.velocity.y_mut() = PLAYER_JUMP_IMPULSE;
			}
		}
	}

	pub fn place_block_where_looking(
		&self,
		cx: &impl BlockPlacementCx,
		world: &mut WorldVoxelData,
		registry: &MaterialRegistry,
		spatial: &Spatial,
		max_dist: f64,
	) {
		let mut ray = RayCast::new_at(
			EntityVoxelPointer::new(world, self.eye_pos(spatial)),
			self.facing.forward().cast(),
		);

		use_generator!(let ray[y] = ray.step_intersect_for(y, cx, storage(), max_dist));

		while let Some((mut isect, _meta)) = ray.next((world, registry)) {
			if isect.block.state(cx, world).is_some_and(|v| v.is_not_air()) {
				isect
					.block
					.at_neighbor(Some((cx, world)), isect.face)
					.set_state_or_warn(
						cx,
						world,
						Block::new(registry.find_by_name("crucible:prototype").unwrap().id),
					);
				break;
			}
		}
	}

	pub fn break_block_where_looking(
		&self,
		cx: &impl BlockPlacementCx,
		world: &mut WorldVoxelData,
		registry: &MaterialRegistry,
		spatial: &Spatial,
		max_dist: f64,
	) {
		let mut ray = RayCast::new_at(
			EntityVoxelPointer::new(world, self.eye_pos(spatial)),
			self.facing.forward().cast(),
		);

		use_generator!(let ray[y] = ray.step_intersect_for(y, cx, storage(), max_dist));

		while let Some((mut isect, _meta)) = ray.next((world, registry)) {
			if isect.block.state(cx, world).is_some_and(|v| v.is_not_air()) {
				isect
					.block
					.set_state_or_warn(cx, world, Block::new(MaterialId::AIR));
				break;
			}
		}
	}
}

#[derive(Debug, Copy, Clone)]
pub struct LocalPlayerInputs {
	pub forward: bool,
	pub backward: bool,
	pub left: bool,
	pub right: bool,
	pub jump: bool,
}
