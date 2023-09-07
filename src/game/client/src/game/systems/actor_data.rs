use bort::{query, BehaviorRegistry, GlobalTag};
use crucible_foundation_shared::actor::{
	manager::ActorManager,
	spatial::{Spatial, SpatialTracker},
};

use super::entry::{ActorSpawnedInGameBehavior, GameInitRegistry, GameSceneInitBehavior};

// === Behaviors === //

pub fn register(bhv: &mut BehaviorRegistry) {
	bhv.register_combined(ActorSpawnedInGameBehavior::new(
		|_bhv, _call_cx, events, scene| {
			let spatial_mgr = &mut *scene.get_mut::<SpatialTracker>();

			query! {
				for (_event in events; omut spatial in GlobalTag::<Spatial>) {
					spatial_mgr.register(&mut spatial);
				}
			}
		},
	));
}

pub fn push_plugins(pm: &mut GameInitRegistry) {
	pm.register(
		[],
		GameSceneInitBehavior::new(|_bhv, _call_cx, scene, _engine| {
			scene.add(SpatialTracker::default());
			scene.add(ActorManager::default());
		}),
	);
}
