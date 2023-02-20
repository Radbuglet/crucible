use bort::{storage, Entity, OwnedEntity};
use crucible_common::{
	game::actor::{ActorManager, Tag},
	voxel::{
		coord::move_rigid_body,
		data::VoxelWorldData,
		math::{Angle3D, Angle3DExt, EntityVec},
	},
};
use crucible_util::debug::error::{ErrorFormatExt, ResultExt};
use typed_glam::glam::Vec3;
use winit::{
	dpi::PhysicalPosition,
	event::{MouseButton, VirtualKeyCode},
	window::CursorGrabMode,
};

use crate::engine::io::{input::InputManager, viewport::Viewport};

// === Factory === //

#[non_exhaustive]
pub struct LocalPlayerTag;

impl Tag for LocalPlayerTag {}

pub fn spawn_player(actors: &mut ActorManager) -> Entity {
	actors.spawn(
		LocalPlayerTag::TAG,
		OwnedEntity::new()
			.with_debug_label("local player")
			.with(LocalPlayerState {
				pos: EntityVec::ZERO,
				vel: EntityVec::ZERO,
				rot: Angle3D::ZERO,
			}),
	)
}

// === Systems === //

pub fn update_local_players(actors: &ActorManager, world: &VoxelWorldData) {
	let states = storage::<LocalPlayerState>();

	for player in actors.tagged::<LocalPlayerTag>() {
		let player_state = &mut *states.get_mut(player);

		// Update velocity
		player_state.vel += EntityVec::NEG_Y;
		player_state.vel *= 0.98;

		// Update position
		move_rigid_body(
			world,
			player_state.pos,
			EntityVec::new(0.8, 1.8, 0.8),
			player_state.vel,
		);
	}
}

#[derive(Debug, Default)]
pub struct PlayerController {
	has_focus: bool,
	local_player: Option<Entity>,
}

impl PlayerController {
	pub fn update(&mut self, main_viewport: Entity) {
		// Acquire context
		let viewport = main_viewport.get::<Viewport>();
		let window = viewport.window();
		let input_manager = main_viewport.get::<InputManager>();

		// Process focus state
		if self.has_focus {
			if input_manager.key(VirtualKeyCode::Escape).recently_pressed() {
				self.has_focus = false;
				window.set_cursor_grab(CursorGrabMode::None).log();
			}
		} else {
			if input_manager.button(MouseButton::Left).recently_pressed() {
				self.has_focus = true;

				// Center mouse
				let window_size = window.inner_size();
				window
					.set_cursor_position(PhysicalPosition::new(
						window_size.width / 2,
						window_size.height / 2,
					))
					.log();

				// Attempt to lock it in place in two different ways
				for mode in [CursorGrabMode::Locked, CursorGrabMode::Confined] {
					match window.set_cursor_grab(mode) {
						Ok(_) => break,
						Err(err) => err.log(),
					}
				}
			}
		}

		// Process local player controls
		if let Some(local_player) = self.local_player.filter(|ent| ent.is_alive()) {
			let mut player_state = local_player.get_mut::<LocalPlayerState>();

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
			player_state.vel += heading * 3.2;

			// Handle jumps
			// TODO: Prevent air jumps
			if input_manager.key(VirtualKeyCode::Space).recently_pressed() {
				player_state.vel = EntityVec::Y * 4.5;
			}
		}
	}
}

// === Components === //

#[derive(Debug)]
pub struct LocalPlayerState {
	pos: EntityVec,
	vel: EntityVec,
	rot: Angle3D,
}
