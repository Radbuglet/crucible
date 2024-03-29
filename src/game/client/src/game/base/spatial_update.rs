use bort::{alias, cx, query, scope, BehaviorRegistry, Cx, GlobalTag};
use crucible_foundation_shared::actor::{
	collider::{Collider, ColliderManager, TrackedCollider},
	spatial::Spatial,
};

use super::behaviors::UpdatePropagateSpatials;

alias! {
	let collider_mgr: ColliderManager;
}

pub fn register(bhv: &mut BehaviorRegistry) {
	bhv.register(UpdatePropagateSpatials::new(|_bhv, s, scene| {
		scope!(
			use let s,
			access cx: Cx<&Spatial, &TrackedCollider, &mut Collider>,
			inject { mut collider_mgr = scene },
		);

		query! {
			for (
				@_me,
				ref spatial in GlobalTag::<Spatial>,
				ref tracked in GlobalTag::<TrackedCollider>,
				omut collider in GlobalTag::<Collider>,
			) {
				let mut aabb = collider.aabb();
				aabb.origin = spatial.pos - tracked.origin_offset;

				#[clippy::accept_danger(direct_collider_access, reason = "this is the system!")]
				collider_mgr.update_aabb(cx!(cx), &mut collider, aabb);
			}
		}
	}));
}
