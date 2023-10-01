use bort::{cx, query, scope, BehaviorRegistry, Cx, GlobalTag};
use crucible_foundation_shared::{
	actor::{collider::Collider, kinematic::KinematicObject, spatial::Spatial},
	math::EntityVec,
	voxel::{collision::MaterialColliderDescriptor, data::ChunkVoxelData},
};

use super::entry::{ActorPhysicsApplyBehavior, ActorPhysicsResetBehavior};

// === Behaviors === //

pub fn register(bhv: &mut BehaviorRegistry) {
	bhv.register(ActorPhysicsResetBehavior::new(|_bhv, s, actor_tag| {
		scope!(use let s);

		query! {
			for (mut kinematic in GlobalTag::<KinematicObject>) + [actor_tag] {
				kinematic.acceleration = EntityVec::ZERO;
			}
		}
	}));

	bhv.register(ActorPhysicsApplyBehavior::new(
		|_bhv, s, actor_tag, world, registry, on_spatial_moved| {
			scope!(
				use let s,
				access cx: Cx<
					&mut Spatial,
					&mut Collider,
					&mut KinematicObject,
					&ChunkVoxelData,
					&MaterialColliderDescriptor,
				>,
			);

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
						cx!(cx),
						world,
						registry,
						&mut spatial,
						collider,
						on_spatial_moved,
						delta,
					);
				}
			}
		},
	));
}
