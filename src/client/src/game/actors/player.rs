use bort::{storage, Entity, OwnedEntity};
use crucible_common::{
	actor::manager::{ActorManager, Tag},
	world::{
		coord::{cast_volume, move_rigid_body, DEFAULT_COLLISION_TOLERANCE},
		data::VoxelWorldData,
		math::{Aabb3, Angle3D, Angle3DExt, BlockFace, EntityVec},
	},
};
use crucible_util::debug::error::{ErrorFormatExt, ResultExt};
use typed_glam::glam::Vec3;
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

const TICKS_TO_FRAMES: f64 = 20.0 / 60.0;
const TICKS_TO_FRAMES_SQ: f64 = TICKS_TO_FRAMES * TICKS_TO_FRAMES;

// See: https://web.archive.org/web/20230313061131/https://www.mcpk.wiki/wiki/Jumping
const PLAYER_SPEED: f64 = 0.13 * TICKS_TO_FRAMES_SQ;
const PLAYER_GRAVITY: f64 = 0.08 * TICKS_TO_FRAMES_SQ;
const PLAYER_AIR_FRICTION_COEF: f64 = 0.98;
const PLAYER_BLOCK_FRICTION_COEF: f64 = 0.91;
const PLAYER_JUMP_IMPULSE: f64 = 0.42 * TICKS_TO_FRAMES;

const PLAYER_WIDTH: f64 = 0.8;
const PLAYER_HEIGHT: f64 = 1.8;
const PLAYER_EYE_LEVEL: f64 = 1.6;

// === Factories === //

#[non_exhaustive]
pub struct LocalPlayerTag;

impl Tag for LocalPlayerTag {}

pub fn spawn_local_player(actors: &mut ActorManager) -> Entity {
	actors.spawn(
		LocalPlayerTag::TAG,
		OwnedEntity::new()
			.with_debug_label("local player")
			.with(LocalPlayerState {
				pos: EntityVec::ZERO,
				vel: EntityVec::ZERO,
				rot: Angle3D::ZERO,
				was_on_ground: true,
			}),
	)
}

// === Components === //

#[derive(Debug)]
pub struct LocalPlayerState {
	pos: EntityVec,
	vel: EntityVec,
	rot: Angle3D,
	was_on_ground: bool,
}

// === Systems === //

pub fn update_local_players(actors: &ActorManager, world: &VoxelWorldData) {
	let states = storage::<LocalPlayerState>();

	for player in actors.tagged::<LocalPlayerTag>() {
		let player_state = &mut *states.get_mut(player);

		// Construct player AABB
		let center_offset = EntityVec::new(PLAYER_WIDTH / 2.0, 0.0, PLAYER_WIDTH / 2.0);
		let aabb = Aabb3 {
			origin: player_state.pos - center_offset,
			size: EntityVec::new(PLAYER_WIDTH, PLAYER_HEIGHT, PLAYER_WIDTH),
		};

		// Check whether we're on the ground
		let on_ground = cast_volume(
			world,
			aabb.quad(BlockFace::NegativeY),
			0.01,
			DEFAULT_COLLISION_TOLERANCE,
		) == 0.0;

		// Update velocity
		if !on_ground {
			player_state.vel += EntityVec::NEG_Y * PLAYER_GRAVITY;
		} else if player_state.vel.y() < 0.0 {
			*player_state.vel.y_mut() = 0.0;
		}

		player_state.was_on_ground = on_ground;
		player_state.vel *= PLAYER_AIR_FRICTION_COEF;
		player_state.vel *=
			EntityVec::new(PLAYER_BLOCK_FRICTION_COEF, 1.0, PLAYER_BLOCK_FRICTION_COEF);

		// Update position
		player_state.pos = move_rigid_body(world, aabb, player_state.vel) + center_offset;
	}
}

#[derive(Debug, Default)]
pub struct PlayerInputController {
	has_focus: bool,
	local_player: Option<Entity>,
	fly_mode: bool,
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
			let mut player_state = local_player.get_mut::<LocalPlayerState>();

			if self.has_focus {
				// Process mouse look
				player_state.rot += input_manager.mouse_delta() * 0.1f32.to_radians();
				player_state.rot = player_state.rot.wrap_x().clamp_y_90();

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
					player_state
						.rot
						.as_matrix_horizontal()
						.transform_vector3(heading)
						.as_dvec3(),
				);

				// Accelerate in that direction
				player_state.vel += heading * PLAYER_SPEED;

				// Process fly mode
				if input_manager.key(VirtualKeyCode::F).recently_pressed() {
					self.fly_mode = !self.fly_mode;
				}

				// Handle jumps
				let should_jump = match self.fly_mode {
					true => input_manager.key(VirtualKeyCode::Space).state(),
					false => {
						input_manager.key(VirtualKeyCode::Space).recently_pressed()
							&& player_state.was_on_ground
					}
				};

				if should_jump {
					*player_state.vel.y_mut() = PLAYER_JUMP_IMPULSE;
				}
			}

			// Attach camera to player head
			camera.set_pos_rot(
				player_state.pos.as_glam().as_vec3() + Vec3::Y * PLAYER_EYE_LEVEL as f32,
				player_state.rot,
				CameraSettings::Perspective {
					fov: 110f32.to_radians(),
					near: 0.1,
					far: 100.0,
				},
			);
		}
	}
}
