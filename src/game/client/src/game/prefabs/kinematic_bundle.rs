use bort::{query, saddle::behavior, BehaviorRegistry, GlobalTag::GlobalTag};
use crucible_foundation_shared::{
	actor::spatial::{self, Spatial},
	math::{kinematic::update_kinematic, Axis3, BlockFace, EntityVec, Sign, VecCompExt},
	voxel::collision::{filter_all_colliders, move_rigid_body},
};
use crucible_util::mem::c_enum::CEnum;

use crate::game::components::kinematic::{self, KinematicSpatial};

use super::scene_root::{ActorPhysicsApplyBehavior, ActorPhysicsResetBehavior};

// === Behaviors === //

pub fn register(bhv: &mut BehaviorRegistry) {
	bhv.register::<ActorPhysicsResetBehavior>(make_physics_reset_behavior())
		.register::<ActorPhysicsApplyBehavior>(make_physics_apply_behavior());
}

fn make_physics_reset_behavior() -> ActorPhysicsResetBehavior {
	ActorPhysicsResetBehavior::new(|_bhv, bhv_cx, actor_tag| {
		behavior! {
			as ActorPhysicsResetBehavior[bhv_cx] do
			(cx: [], _bhv_cx: []) {
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
	ActorPhysicsApplyBehavior::new(|_bhv, bhv_cx, actor_tag, spatial_mgr, world, registry| {
		behavior! {
			as ActorPhysicsApplyBehavior[bhv_cx] do
			(cx: [spatial::CxMut, kinematic::CxMut], _bhv_cx: []) {
				query! {
					for (
						@_me,
						omut spatial in GlobalTag::<Spatial>,
						mut kinematic in GlobalTag::<KinematicSpatial>,
					) + [actor_tag] {
						// Clip velocities and accelerations into obstructed faces
						kinematic.update_face_touching_mask(cx, world, registry, &spatial);

						for axis in Axis3::variants() {
							let clip_comp = |comp: &mut f64| {
								let sign = Sign::of(*comp).unwrap_or(Sign::Positive);
								let face = BlockFace::compose(axis, sign);

								if kinematic.collision_mask[face] {
									*comp = 0.0;
								}
							};

							// N.B. we do these separately because a player could be accelerating
							// in the direction opposite to which they are moving.
							clip_comp(kinematic.velocity.comp_mut(axis));
							clip_comp(kinematic.acceleration.comp_mut(axis));
						}

						// Update velocity and position
						let aabb = spatial.aabb();
						let (delta_pos, velocity) = update_kinematic(
							kinematic.velocity,
							kinematic.acceleration,
							kinematic.friction,
							1. / 60.,  // TODO: use real delta
						);

						kinematic.velocity = velocity;
						let new_origin = move_rigid_body(cx, world, registry, aabb, delta_pos, filter_all_colliders());

						spatial_mgr.update(cx, &mut spatial, aabb.with_origin(new_origin));
					}
				}
			}
		}
	})
}
