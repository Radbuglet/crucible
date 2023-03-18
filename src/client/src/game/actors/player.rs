use bort::{storage, Entity, OwnedEntity};
use crucible_common::{
	actor::{
		kinds::spatial::{KinematicSpatial, KinematicUpdateTag, Spatial},
		kinematic::{tick_friction_coef_to_coef_qty, MC_TICKS_TO_SECS, MC_TICKS_TO_SECS_SQUARED},
		manager::{ActorManager, Tag},
	},
	world::{
		data::VoxelWorldData,
		math::{Aabb3, Angle3D, Angle3DExt, BlockFace, EntityVec},
	},
};
use crucible_util::debug::error::{ErrorFormatExt, ResultExt};
use typed_glam::glam::{DVec3, Vec3};
use winit::{
	dpi::PhysicalPosition,
	event::{MouseButton, VirtualKeyCode},
	window::CursorGrabMode,
};

use crate::engine::{
	gfx::camera::{CameraManager, CameraSettings},
	io::{input::InputManager, viewport::Viewport},
};

// === Constants === //

// See: https://web.archive.org/web/20230313061131/https://www.mcpk.wiki/wiki/Jumping
pub const GRAVITY: f64 = 0.08 * MC_TICKS_TO_SECS_SQUARED;
pub const GRAVITY_VEC: EntityVec = EntityVec::from_glam(DVec3::new(0.0, -GRAVITY, 0.0));

const PLAYER_SPEED: f64 = 0.13 * MC_TICKS_TO_SECS_SQUARED;
const PLAYER_AIR_FRICTION_COEF: f64 = 0.98;
const PLAYER_BLOCK_FRICTION_COEF: f64 = 0.91;

const PLAYER_JUMP_IMPULSE: f64 = 0.42 * MC_TICKS_TO_SECS;
const PLAYER_JUMP_COOL_DOWN: u64 = 30;

const PLAYER_WIDTH: f64 = 0.8;
const PLAYER_HEIGHT: f64 = 1.8;
const PLAYER_EYE_LEVEL: f64 = 1.6;

// === Factories === //

pub fn spawn_local_player(actors: &mut ActorManager) -> Entity {
	actors.spawn(
		KinematicUpdateTag::TAG,
		OwnedEntity::new()
			.with_debug_label("local player")
			.with(Spatial {
				position: EntityVec::ZERO,
				rotation: Angle3D::ZERO,
			})
			.with(KinematicSpatial::new(
				Aabb3 {
					origin: EntityVec::ZERO,
					size: EntityVec::new(PLAYER_WIDTH, PLAYER_HEIGHT, PLAYER_WIDTH),
				}
				.centered_at(EntityVec::new(0.5, 0.0, 0.5)),
				tick_friction_coef_to_coef_qty(
					EntityVec::new(
						PLAYER_AIR_FRICTION_COEF * PLAYER_BLOCK_FRICTION_COEF,
						PLAYER_AIR_FRICTION_COEF,
						PLAYER_AIR_FRICTION_COEF * PLAYER_BLOCK_FRICTION_COEF,
					),
					60.0,
				),
			)),
	)
}

// TODO: This belongs somewhere else.
pub fn reset_kinematic_accelerations_to_gravity(actors: &ActorManager) {
	let kinematics = storage::<KinematicSpatial>();

	for actor in actors.tagged::<KinematicUpdateTag>() {
		kinematics.get_mut(actor).acceleration = GRAVITY_VEC;
	}
}

// === Components === //

#[derive(Debug, Default)]
pub struct PlayerInputController {
	local_player: Option<Entity>,
	has_focus: bool,
	fly_mode: bool,
	jump_cool_down: u64,
}

impl PlayerInputController {
	pub fn local_player(&self) -> Option<Entity> {
		self.local_player
	}

	pub fn set_local_player(&mut self, player: Option<Entity>) {
		self.local_player = player;
	}

	pub fn update(
		&mut self,
		main_viewport: Entity,
		camera: &mut CameraManager,
		_world: &mut VoxelWorldData,
	) {
		// Acquire context
		let viewport = main_viewport.get::<Viewport>();
		let window = viewport.window();
		let input_manager = main_viewport.get::<InputManager>();

		// Process focus state
		if self.has_focus {
			if input_manager.key(VirtualKeyCode::Escape).recently_pressed() {
				self.has_focus = false;

				// Release cursor grab
				window.set_cursor_grab(CursorGrabMode::None).log();

				// Show the cursor
				window.set_cursor_visible(true);
			}
		} else {
			if input_manager.button(MouseButton::Left).recently_pressed() {
				self.has_focus = true;

				// Center cursor
				let window_size = window.inner_size();
				window
					.set_cursor_position(PhysicalPosition::new(
						window_size.width / 2,
						window_size.height / 2,
					))
					.log();

				// Attempt to lock cursor in place in two different ways
				for mode in [CursorGrabMode::Locked, CursorGrabMode::Confined] {
					match window.set_cursor_grab(mode) {
						Ok(_) => break,
						Err(err) => err.log(),
					}
				}

				// Hide the cursor
				window.set_cursor_visible(false);
			}
		}

		// Process local player controls
		if let Some(local_player) = self.local_player.filter(|ent| ent.is_alive()) {
			let player_spatial = &mut *local_player.get_mut::<Spatial>();
			let player_kinematic = &mut *local_player.get_mut::<KinematicSpatial>();

			if self.has_focus {
				// Process mouse look
				player_spatial.rotation += input_manager.mouse_delta() * 0.1f32.to_radians();
				player_spatial.rotation = player_spatial.rotation.wrap_x().clamp_y_90();

				// Compute heading
				let mut heading = Vec3::ZERO;

				if input_manager.key(VirtualKeyCode::W).state() {
					heading += Vec3::Z;
				}

				if input_manager.key(VirtualKeyCode::S).state() {
					heading -= Vec3::Z;
				}

				if input_manager.key(VirtualKeyCode::A).state() {
					heading -= Vec3::X;
				}

				if input_manager.key(VirtualKeyCode::D).state() {
					heading += Vec3::X;
				}

				// Normalize heading
				let heading = heading.normalize_or_zero();

				// Convert player-local heading to world space
				let heading = EntityVec::cast_from(
					player_spatial
						.rotation
						.as_matrix_horizontal()
						.transform_vector3(heading)
						.as_dvec3(),
				);

				// Accelerate in that direction
				player_kinematic.apply_acceleration(heading * PLAYER_SPEED);

				// Process fly mode
				if input_manager.key(VirtualKeyCode::F).recently_pressed() {
					self.fly_mode = !self.fly_mode;
				}

				// Handle jumps
				let space_pressed = input_manager.key(VirtualKeyCode::Space).state();

				if !space_pressed {
					self.jump_cool_down = 0;
				}

				if self.jump_cool_down > 0 {
					self.jump_cool_down -= 1;
				}

				if space_pressed {
					if self.fly_mode {
						player_kinematic.apply_acceleration(-GRAVITY_VEC * 2.0);
					} else if self.jump_cool_down == 0
						&& player_kinematic.was_face_touching(BlockFace::NegativeY)
					{
						self.jump_cool_down = PLAYER_JUMP_COOL_DOWN;
						*player_kinematic.velocity.y_mut() = PLAYER_JUMP_IMPULSE;
					}
				}
			}

			// Attach camera to player head
			camera.set_pos_rot(
				player_spatial.position.as_glam().as_vec3() + Vec3::Y * PLAYER_EYE_LEVEL as f32,
				player_spatial.rotation,
				CameraSettings::Perspective {
					fov: 110f32.to_radians(),
					near: 0.1,
					far: 100.0,
				},
			);
		}
	}
}
