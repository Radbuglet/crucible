use bort::{query, BehaviorRegistry, GlobalTag::GlobalTag};
use crucible_foundation_shared::actor::spatial::{Spatial, SpatialTracker};

use super::scene_root::ActorSpawnedInGameBehavior;

pub fn register(bhv: &mut BehaviorRegistry) {
	bhv.register::<ActorSpawnedInGameBehavior>(ActorSpawnedInGameBehavior::new(
		|_bhv, events, scene| {
			let spatial_mgr = &mut *scene.get_mut::<SpatialTracker>();

			query! {
				for (_event in *events; omut spatial in GlobalTag::<Spatial>) {
					spatial_mgr.register(&mut spatial);
				}
			}
		},
	));
}