use bort::{query, saddle::behavior, BehaviorRegistry, GlobalTag, OwnedEntity};
use crucible_foundation_client::engine::gfx::camera::CameraSettings;
use crucible_foundation_shared::{
	actor::spatial::Spatial,
	math::{Aabb3, Angle3D, Angle3DExt, EntityVec},
};

use crate::game::components::player::LocalPlayer;

use super::{
	scene_root::{ActorInputBehavior, ActorSpawnedInGameBehavior, CameraProviderBehavior},
	spatial_bundle::push_spatial_bundle,
};

// === Prefabs === //

pub fn make_local_player() -> OwnedEntity {
	OwnedEntity::new()
		.with_debug_label("local player")
		.with_tagged(
			GlobalTag::<LocalPlayer>,
			LocalPlayer {
				facing: Angle3D::ZERO,
			},
		)
		.with_many(|ent| {
			push_spatial_bundle(
				ent,
				Aabb3::from_origin_size(
					EntityVec::ZERO,
					EntityVec::new(1.0, 2.0, 1.0),
					EntityVec::new(0.5, 0.0, 0.5),
				),
			)
		})
}

pub fn register(bhv: &mut BehaviorRegistry) {
	bhv.register::<ActorSpawnedInGameBehavior>(ActorSpawnedInGameBehavior::new(
		|_bhv, events, _scene| {
			query! {
				for (_event in *events; @me) + [GlobalTag::<LocalPlayer>] {
					log::info!("Spawned player {me:?}");
				}
			}
		},
	))
	.register::<ActorInputBehavior>(ActorInputBehavior::new(
		|_bhv, bhv_cx, actor_tag, inputs| {
			behavior! {
				as ActorInputBehavior[bhv_cx] do
				(_cx: [;mut LocalPlayer], _bhv_cx: []) {
					query! {
						for (mut player in GlobalTag::<LocalPlayer>) + [actor_tag] {
							player.facing += inputs.mouse_delta() * f32::to_radians(0.4);
							player.facing = player.facing.clamp_y_90().wrap_x();
						}
					}
				}
			}
		},
	))
	.register::<CameraProviderBehavior>(CameraProviderBehavior::new(
		|_bhv, bhv_cx, actor_tag, camera_mgr| {
			behavior! {
				as CameraProviderBehavior[bhv_cx] do
				(_cx: [; ref Spatial, ref LocalPlayer], _bhv_cx: []) {
					query! {
						for (ref spatial in GlobalTag::<Spatial>, ref player in GlobalTag::<LocalPlayer>) + [actor_tag] {
							camera_mgr.set_pos_rot(
								spatial.aabb().at_percent(EntityVec::new(0.5, 0.9, 0.5)).to_glam().as_vec3(),
								player.facing,
								CameraSettings::Perspective { fov: 70f32.to_radians(), near: 0.1, far: 100.0 },
							);
						}
					}
				}
			}
		},
	));
}
