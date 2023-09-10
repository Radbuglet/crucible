use bort::{proc, query, BehaviorRegistry, GlobalTag::GlobalTag};
use crucible_foundation_shared::{
	actor::{
		collider::Collider,
		kinematic::{self, KinematicObject},
		spatial::Spatial,
	},
	math::EntityVec,
};

use super::entry::{ActorPhysicsApplyBehavior, ActorPhysicsResetBehavior};

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
					for (mut kinematic in GlobalTag::<KinematicObject>) + [actor_tag] {
						kinematic.acceleration = EntityVec::ZERO;
					}
				}
			}
		}
	})
}

fn make_physics_apply_behavior() -> ActorPhysicsApplyBehavior {
	ActorPhysicsApplyBehavior::new(
		|_bhv, call_cx, actor_tag, world, registry, on_spatial_moved| {
			proc! {
				as ActorPhysicsApplyBehavior[call_cx] do
				(
					cx: [mut Spatial, ref Collider, mut KinematicObject; kinematic::CxApplyPhysics],
					_call_cx: [],
				) {
					// TODO: Compute this.
					let delta = 1.0 / 60.0;

					query! {
						for (
							@_me,
							omut spatial in GlobalTag::<Spatial>,
							ref collider in GlobalTag::<Collider>,
							mut kinematic in GlobalTag::<KinematicObject>,
						) + [actor_tag] {
							kinematic.apply_physics(
								cx,
								world,
								registry,
								&mut spatial,
								collider,
								on_spatial_moved,
								delta,
							);
						}
					}
				}
			}
		},
	)
}
