use bort::{cx, query, scope, BehaviorRegistry, Cx, GlobalTag, VecEventList};
use crucible_foundation_shared::actor::{
	collider::{Collider, ColliderManager},
	manager::{ActorManager, ActorSpawned},
};
use crucible_util::debug::type_id::NamedTypeId;

use super::behaviors::{InitGame, UpdateHandleEarlyEvents};

pub fn register(bhv: &mut BehaviorRegistry) {
	bhv.register_cx(
		[],
		InitGame::new(|_bhv, s, scene, _engine| {
			scope!(use let s);

			scene.add(ColliderManager::default());
			scene.add(ActorManager::default());
		}),
	);

	bhv.register_cx(
		([NamedTypeId::of::<ActorSpawned>()], []),
		UpdateHandleEarlyEvents::new(|_bhv, s, events, scene| {
			scope!(
				use let s,
				access cx: Cx<&mut Collider, &VecEventList<ActorSpawned>>,
				inject { mut collider_mgr as ColliderManager = scene },
			);

			query! {
				for (_event in events.get_s::<ActorSpawned>(cx!(cx)); omut collider in GlobalTag::<Collider>) {
					#[clippy::accept_danger(direct_collider_access, reason = "this is that system!")]
					collider_mgr.register(&mut collider);
				}
			}
		}),
	);
}
