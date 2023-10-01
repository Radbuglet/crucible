use bort::{query, scope, BehaviorRegistry, Cx, GlobalTag};
use crucible_foundation_shared::actor::{
	collider::{Collider, ColliderManager},
	manager::ActorManager,
};

use super::entry::{ActorSpawnedInGameBehavior, GameSceneInitBehavior};

pub fn register(bhv: &mut BehaviorRegistry) {
	bhv.register_cx(
		[],
		GameSceneInitBehavior::new(|_bhv, s, scene, _engine| {
			scope!(use let s);

			scene.add(ColliderManager::default());
			scene.add(ActorManager::default());
		}),
	);

	bhv.register(ActorSpawnedInGameBehavior::new(
		|_bhv, s, on_spawn, scene| {
			scope!(
				use let s,
				access _cx: Cx<&mut Collider>,
				inject { mut collider_mgr as ColliderManager = scene },
			);

			query! {
				for (_event in on_spawn; omut collider in GlobalTag::<Collider>) {
					collider_mgr.register(&mut collider);
				}
			}
		},
	));
}
