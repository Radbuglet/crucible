use bort::{cx, alias, query, scope, BehaviorRegistry, Cx, GlobalTag};
use crucible_foundation_shared::actor::{
	collider::{Collider, ColliderManager, TrackedCollider},
	spatial::Spatial,
};

use super::entry::SpatialUpdateApplyUpdates;

// === Behaviors === //

alias! {
	let collider_mgr: ColliderManager;
}

pub fn register(bhv: &mut BehaviorRegistry) {
	bhv.register_combined(SpatialUpdateApplyUpdates::new(
		|_bhv, s, scene, on_spatial_moved| {
			scope!(
				use let s,
				access cx: Cx<&Spatial, &TrackedCollider, &mut Collider>,
				inject { mut collider_mgr = scene },
			);

			query! {
				for (
					_ev in on_spatial_moved;
					ref spatial in GlobalTag::<Spatial>,
					ref tracked in GlobalTag::<TrackedCollider>,
					omut collider in GlobalTag::<Collider>,
				) {
					let mut aabb = collider.aabb();
					aabb.origin = spatial.pos() - tracked.origin_offset;
					collider_mgr.update_aabb_directly(cx!(cx), &mut collider, aabb);
				}
			}
		},
	));
}
