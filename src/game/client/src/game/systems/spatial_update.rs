use bort::{alias, proc, query, BehaviorRegistry, GlobalTag};
use crucible_foundation_shared::actor::{
	collider::{Collider, ColliderManager, ColliderMutateCx, TrackedCollider},
	spatial::Spatial,
};

use super::entry::SpatialUpdateApplyUpdates;

// === Behaviors === //

alias! {
	let collider_mgr: ColliderManager;
}

pub fn register(bhv: &mut BehaviorRegistry) {
	bhv.register_combined(SpatialUpdateApplyUpdates::new(
		|_bhv, call_cx, scene, on_spatial_moved| {
			proc! {
				as SpatialUpdateApplyUpdates[call_cx] do
				(
					cx: [ref Spatial, ref TrackedCollider, mut Collider; ColliderMutateCx],
					_call_cx: [],
					mut collider_mgr = scene,
				) {
					query! {
						for (
							_ev in on_spatial_moved;
							ref spatial in GlobalTag::<Spatial>,
							ref tracked in GlobalTag::<TrackedCollider>,
							omut collider in GlobalTag::<Collider>,
						) {
							let mut aabb = collider.aabb();
							aabb.origin = spatial.pos() - tracked.origin_offset;
							collider_mgr.update_aabb_directly(cx, &mut collider, aabb);
						}
					}
				}
			}
		},
	));
}
