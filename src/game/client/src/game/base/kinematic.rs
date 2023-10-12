use bort::{cx, query, scope, BehaviorRegistry, Cx, GlobalTag};
use crucible_foundation_shared::{
	actor::{collider::Collider, kinematic::KinematicObject, spatial::Spatial},
	math::EntityVec,
	voxel::{collision::MaterialColliderDescriptor, data::ChunkVoxelData},
};

use super::behaviors::{UpdateApplyPhysics, UpdateTickReset};

pub fn register(bhv: &mut BehaviorRegistry) {
	bhv.register(UpdateTickReset::new(|_bhv, s, _events, actor_tag| {
		scope!(use let s);

		query! {
			for (mut kinematic in GlobalTag::<KinematicObject>) + [actor_tag] {
				kinematic.acceleration = EntityVec::ZERO;
			}
		}
	}));

	bhv.register(UpdateApplyPhysics::new(
		|_bhv, s, actor_tag, world, registry| {
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
					mut spatial in GlobalTag::<Spatial>,
					ref collider in GlobalTag::<Collider>,
					mut kinematic in GlobalTag::<KinematicObject>,
				) + [actor_tag] {
					#[clippy::accept_danger(direct_kinematic_updating, reason = "this is that system!")]
					kinematic.apply_physics(
						cx!(cx),
						world,
						registry,
						spatial,
						collider,
						delta,
					);
				}
			}
		},
	));
}
