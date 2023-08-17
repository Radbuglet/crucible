use bort::{
	query, saddle::behavior, BehaviorRegistry, GlobalTag, GlobalVirtualTag, HasGlobalVirtualTag,
	OwnedEntity,
};
use crucible_foundation_client::engine::gfx::camera::CameraSettings;
use crucible_foundation_shared::{
	actor::spatial::Spatial,
	math::{Aabb3, Angle3D, EntityVec},
};

use super::{
	scene_root::{ActorSpawnedInGameBehavior, CameraProviderDelegate},
	spatial_bundle::push_spatial_bundle,
};

// === Tags === //

pub struct LocalPlayerTag;

impl HasGlobalVirtualTag for LocalPlayerTag {}

// === Prefabs === //

pub fn make_local_player() -> OwnedEntity {
	OwnedEntity::new()
		.with_debug_label("local player")
		.with_tag(GlobalVirtualTag::<LocalPlayerTag>)
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
				for (_event in *events; @me) + [GlobalVirtualTag::<LocalPlayerTag>] {
					log::info!("Spawned player {me:?}");
				}
			}
		},
	))
	.register::<CameraProviderDelegate>(CameraProviderDelegate::new(
		|_bhv, bhv_cx, actor_tag, camera_mgr| {
			behavior! {
				as CameraProviderDelegate[bhv_cx] do
				(cx: [;ref Spatial], _bhv_cx: []) {
					query! {
						for (ref spatial in GlobalTag::<Spatial>) + [actor_tag, GlobalVirtualTag::<LocalPlayerTag>] {
							camera_mgr.set_pos_rot(
								spatial.aabb().at_percent(EntityVec::new(0.5, 0.9, 0.5)).to_glam().as_vec3(),
								Angle3D::new(0.0, 0.0),
								CameraSettings::Perspective { fov: 70f32.to_radians(), near: 0.1, far: 100.0 },
							);
						}
					}
				}
			}
		},
	));
}
