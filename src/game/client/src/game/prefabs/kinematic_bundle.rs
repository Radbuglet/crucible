use bort::{proc, query, BehaviorRegistry, GlobalTag::GlobalTag};
use crucible_foundation_shared::{actor::spatial::Spatial, math::EntityVec};

use crate::game::components::kinematic::{self, KinematicSpatial};

use super::scene_root::{ActorPhysicsApplyBehavior, ActorPhysicsResetBehavior};

// === Behaviors === //

pub fn register(bhv: &mut BehaviorRegistry) {
	bhv.register_combined(make_physics_reset_behavior())
		.register_combined(make_physics_apply_behavior());
}

fn make_physics_reset_behavior() -> ActorPhysicsResetBehavior {
	ActorPhysicsResetBehavior::new(|_bhv, call_cx, actor_tag| {
		proc! {
			as ActorPhysicsResetBehavior[call_cx] do
			(cx: [], _call_cx: []) {
				query! {
					for (mut kinematic in GlobalTag::<KinematicSpatial>) + [actor_tag] {
						kinematic.acceleration = EntityVec::ZERO;
					}
				}
			}
		}
	})
}

fn make_physics_apply_behavior() -> ActorPhysicsApplyBehavior {
	ActorPhysicsApplyBehavior::new(|_bhv, call_cx, actor_tag, spatial_mgr, world, registry| {
		proc! {
			as ActorPhysicsApplyBehavior[call_cx] do
			(cx: [;kinematic::CxApplyPhysics], _call_cx: []) {
				// TODO: Compute this.
				let delta = 1.0 / 60.0;

				query! {
					for (
						@_me,
						omut spatial in GlobalTag::<Spatial>,
						mut kinematic in GlobalTag::<KinematicSpatial>,
					) + [actor_tag] {
						kinematic.apply_physics(
							cx,
							world,
							registry,
							spatial_mgr,
							&mut spatial,
							delta,
						);
					}
				}
			}
		}
	})
}
