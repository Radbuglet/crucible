use bort::{query, BehaviorRegistry, GlobalVirtualTag, HasGlobalVirtualTag, OwnedEntity};
use crucible_foundation_shared::math::{Aabb3, EntityVec};

use super::{scene_root::ActorSpawnedInGameBehavior, spatial_bundle::push_spatial_bundle};

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
	));
}
