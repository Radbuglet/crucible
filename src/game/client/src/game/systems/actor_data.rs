use bort::{query, BehaviorRegistry, GlobalTag};
use crucible_foundation_shared::actor::{
	collider::{Collider, ColliderManager},
	manager::ActorManager,
};

use super::entry::{ActorSpawnedInGameBehavior, GameInitRegistry, GameSceneInitBehavior};

// === Behaviors === //

pub fn register(bhv: &mut BehaviorRegistry) {
	bhv.register_combined(make_actor_spawn_handler());
}

fn make_actor_spawn_handler() -> ActorSpawnedInGameBehavior {
	ActorSpawnedInGameBehavior::new(|_bhv, _call_cx, on_spawn, scene| {
		let collider_mgr = &mut *scene.get_mut::<ColliderManager>();

		query! {
			for (_event in on_spawn; omut collider in GlobalTag::<Collider>) {
				collider_mgr.register(&mut collider);
			}
		}
	})
}

pub fn push_plugins(pm: &mut GameInitRegistry) {
	pm.register(
		[],
		GameSceneInitBehavior::new(|_bhv, _call_cx, scene, _engine| {
			scene.add(ColliderManager::default());
			scene.add(ActorManager::default());
		}),
	);
}
