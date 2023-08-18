use bort::{query, saddle::behavior, BehaviorRegistry, GlobalTag, OwnedEntity};
use crucible_foundation_client::engine::gfx::camera::CameraSettings;
use crucible_foundation_shared::{
	actor::spatial::Spatial,
	math::{
		kinematic::{tick_friction_coef_to_coef_qty, MC_TICKS_TO_SECS, MC_TICKS_TO_SECS_SQUARED},
		Aabb3, Angle3D, Angle3DExt, BlockFace, EntityVec,
	},
};
use typed_glam::glam::{DVec3, Vec3};
use winit::event::VirtualKeyCode;

use crate::game::components::{kinematic::KinematicSpatial, player::LocalPlayer};

use super::scene_root::{ActorInputBehavior, ActorSpawnedInGameBehavior, CameraProviderBehavior};

// === Constants === //

// See: https://web.archive.org/web/20230313061131/https://www.mcpk.wiki/wiki/Jumping
pub const GRAVITY: f64 = 0.08 * MC_TICKS_TO_SECS_SQUARED;
pub const GRAVITY_VEC: EntityVec = EntityVec::from_glam(DVec3::new(0.0, -GRAVITY, 0.0));

const PLAYER_SPEED: f64 = 0.13 * MC_TICKS_TO_SECS_SQUARED;
const PLAYER_AIR_FRICTION_COEF: f64 = 0.98;
const PLAYER_BLOCK_FRICTION_COEF: f64 = 0.91;

const PLAYER_JUMP_IMPULSE: f64 = 0.43 * MC_TICKS_TO_SECS;
const PLAYER_JUMP_COOL_DOWN: u64 = 30;

const PLAYER_WIDTH: f64 = 0.8;
const PLAYER_HEIGHT: f64 = 1.8;
const PLAYER_EYE_LEVEL: f64 = 1.6;

// === Prefabs === //

pub fn make_local_player() -> OwnedEntity {
	OwnedEntity::new()
		.with_debug_label("local player")
		.with_tagged(
			GlobalTag::<LocalPlayer>,
			LocalPlayer {
				facing: Angle3D::ZERO,
				fly_mode: false,
				jump_cool_down: 0,
			},
		)
		.with_tagged(
			GlobalTag::<Spatial>,
			Spatial::new(Aabb3::from_origin_size(
				EntityVec::ZERO,
				EntityVec::new(PLAYER_WIDTH, PLAYER_HEIGHT, PLAYER_WIDTH),
				EntityVec::new(0.5, 0.0, 0.5),
			)),
		)
		.with_tagged(
			GlobalTag::<KinematicSpatial>,
			KinematicSpatial::new(tick_friction_coef_to_coef_qty(
				EntityVec::new(
					PLAYER_AIR_FRICTION_COEF * PLAYER_BLOCK_FRICTION_COEF,
					PLAYER_AIR_FRICTION_COEF,
					PLAYER_AIR_FRICTION_COEF * PLAYER_BLOCK_FRICTION_COEF,
				),
				60.0,
			)),
		)
}

// === Behaviors === //

pub fn register(bhv: &mut BehaviorRegistry) {
	bhv.register::<ActorSpawnedInGameBehavior>(make_spawn_behavior())
		.register::<ActorInputBehavior>(make_input_behavior())
		.register::<CameraProviderBehavior>(make_camera_behavior());
}

fn make_spawn_behavior() -> ActorSpawnedInGameBehavior {
	ActorSpawnedInGameBehavior::new(|_bhv, events, _scene| {
		query! {
			for (_event in *events; @me) + [GlobalTag::<LocalPlayer>] {
				log::info!("Spawned player {me:?}");
			}
		}
	})
}

fn make_input_behavior() -> ActorInputBehavior {
	ActorInputBehavior::new(|_bhv, bhv_cx, actor_tag, inputs| {
		behavior! {
			as ActorInputBehavior[bhv_cx] do
			(_cx: [;mut LocalPlayer, mut KinematicSpatial], _bhv_cx: []) {
				query! {
					for (
						mut player in GlobalTag::<LocalPlayer>,
						mut kinematic in GlobalTag::<KinematicSpatial>,
					) + [actor_tag] {
						// Apply gravity
						kinematic.apply_acceleration(GRAVITY_VEC);

						// Process mouse look
						player.facing += inputs.mouse_delta() * f32::to_radians(0.4);
						player.facing = player.facing.clamp_y_90().wrap_x();

						// Compute heading
						let mut heading = Vec3::ZERO;

						if inputs.key(VirtualKeyCode::W).state() {
							heading += Vec3::Z;
						}

						if inputs.key(VirtualKeyCode::S).state() {
							heading -= Vec3::Z;
						}

						if inputs.key(VirtualKeyCode::A).state() {
							heading -= Vec3::X;
						}

						if inputs.key(VirtualKeyCode::D).state() {
							heading += Vec3::X;
						}

						// Normalize heading
						let heading = heading.normalize_or_zero();

						// Convert player-local heading to world space
						let heading = EntityVec::cast_from(
							player
								.facing
								.as_matrix_horizontal()
								.transform_vector3(heading),
						);

						// Accelerate in that direction
						kinematic.apply_acceleration(heading * PLAYER_SPEED);

						// Process fly mode
						if inputs.key(VirtualKeyCode::F).recently_pressed() {
							player.fly_mode = !player.fly_mode;
						}

						// Handle jumps
						let space_pressed = inputs.key(VirtualKeyCode::Space).state();

						if !space_pressed {
							player.jump_cool_down = 0;
						}

						if player.jump_cool_down > 0 {
							player.jump_cool_down -= 1;
						}

						if space_pressed {
							if player.fly_mode {
								kinematic.apply_acceleration(-GRAVITY_VEC * 2.0);
							} else if player.jump_cool_down == 0
								&& kinematic.was_face_touching(BlockFace::NegativeY)
							{
								player.jump_cool_down = PLAYER_JUMP_COOL_DOWN;
								*kinematic.velocity.y_mut() = PLAYER_JUMP_IMPULSE;
							}
						}
					}
				}
			}
		}
	})
}

fn make_camera_behavior() -> CameraProviderBehavior {
	CameraProviderBehavior::new(|_bhv, bhv_cx, actor_tag, camera_mgr| {
		behavior! {
			as CameraProviderBehavior[bhv_cx] do
			(_cx: [; ref Spatial, ref LocalPlayer], _bhv_cx: []) {
				query! {
					for (ref spatial in GlobalTag::<Spatial>, ref player in GlobalTag::<LocalPlayer>) + [actor_tag] {
						camera_mgr.set_pos_rot(
							spatial.aabb()
								.at_percent(EntityVec::new(0.5, PLAYER_EYE_LEVEL / PLAYER_HEIGHT, 0.5))
								.to_glam().as_vec3(),
							player.facing,
							CameraSettings::Perspective { fov: 110f32.to_radians(), near: 0.1, far: 100.0 },
						);
					}
				}
			}
		}
	})
}
