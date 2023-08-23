use bort::{alias, proc, query, BehaviorRegistry, GlobalTag, OwnedEntity};
use crucible_foundation_client::engine::gfx::camera::CameraSettings;
use crucible_foundation_shared::{
	actor::spatial::Spatial,
	material::MaterialRegistry,
	math::{kinematic::tick_friction_coef_to_coef_qty, Angle3D, Angle3DExt, EntityVec},
	voxel::{
		collision::{self},
		data::WorldVoxelData,
	},
};
use winit::event::{MouseButton, VirtualKeyCode};

use crate::game::components::{
	kinematic::KinematicSpatial,
	player::{
		BlockPlacementCx, LocalPlayer, LocalPlayerInputs, GRAVITY_VEC, PLAYER_AIR_FRICTION_COEF,
		PLAYER_BLOCK_FRICTION_COEF,
	},
};

use super::scene_root::{ActorInputBehavior, ActorSpawnedInGameBehavior, CameraProviderBehavior};

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
				view_bob: 0.0,
			},
		)
		.with_tagged(
			GlobalTag::<Spatial>,
			Spatial::new(LocalPlayer::new_aabb(EntityVec::ZERO)),
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

alias! {
	let registry: MaterialRegistry;
	let world: WorldVoxelData;
}

pub fn register(bhv: &mut BehaviorRegistry) {
	bhv.register_combined(make_spawn_behavior())
		.register_combined(make_input_behavior())
		.register_combined(make_camera_behavior());
}

fn make_spawn_behavior() -> ActorSpawnedInGameBehavior {
	ActorSpawnedInGameBehavior::new(|_bhv, _call_cx, events, _scene| {
		query! {
			for (_event in *events; @me) + [GlobalTag::<LocalPlayer>] {
				log::info!("Spawned player {me:?}");
			}
		}
	})
}

fn make_input_behavior() -> ActorInputBehavior {
	ActorInputBehavior::new(|_bhv, call_cx, scene_root, actor_tag, inputs| {
		proc! {
			as ActorInputBehavior[call_cx] do
			(
				cx: [
					mut LocalPlayer,
					ref Spatial,
					mut KinematicSpatial
					; collision::ColliderCheckCx, BlockPlacementCx
				],
				_call_cx: [],
				mut world = scene_root,
				ref registry = scene_root,
			) {{
				query! {
					for (
						mut player in GlobalTag::<LocalPlayer>,
						ref spatial in GlobalTag::<Spatial>,
						mut kinematic in GlobalTag::<KinematicSpatial>,
					) + [actor_tag] {
						// Apply gravity
						kinematic.apply_acceleration(GRAVITY_VEC);

						// Process mouse look
						player.facing += inputs.mouse_delta() * f32::to_radians(0.4);
						player.facing = player.facing.clamp_y_90().wrap_x();

						// Process fly mode
						if inputs.key(VirtualKeyCode::F).recently_pressed() {
							player.fly_mode = !player.fly_mode;
						}

						// Process movement
						player.process_movement(kinematic, LocalPlayerInputs {
							forward: inputs.key(VirtualKeyCode::W).state(),
							backward: inputs.key(VirtualKeyCode::S).state(),
							left: inputs.key(VirtualKeyCode::A).state(),
							right: inputs.key(VirtualKeyCode::D).state(),
							jump: inputs.key(VirtualKeyCode::Space).state(),
						});

						// Handle block placement
						if inputs.button(MouseButton::Right).recently_pressed() {
							player.place_block_where_looking(cx, world, registry, spatial, 7.0);
						}

						if inputs.button(MouseButton::Left).recently_pressed() {
							player.break_block_where_looking(cx, world, registry, spatial, 7.0);
						}
					}
				}
			}}
		}
	})
}

fn make_camera_behavior() -> CameraProviderBehavior {
	CameraProviderBehavior::new(|_bhv, call_cx, actor_tag, camera_mgr| {
		proc! {
			as CameraProviderBehavior[call_cx] do
			(_cx: [ref Spatial, ref LocalPlayer], _call_cx: []) {
				query! {
					for (ref spatial in GlobalTag::<Spatial>, ref player in GlobalTag::<LocalPlayer>) + [actor_tag] {
						camera_mgr.set_pos_rot(
							player.eye_pos(spatial).to_glam().as_vec3(),
							player.facing,
							CameraSettings::Perspective { fov: 110f32.to_radians(), near: 0.1, far: 500.0 },
						);
					}
				}
			}
		}
	})
}
