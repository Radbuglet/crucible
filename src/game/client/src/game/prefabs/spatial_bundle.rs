use bort::{
	query, BehaviorRegistry, Entity, GlobalTag::GlobalTag, GlobalVirtualTag, HasGlobalVirtualTag,
};
use crucible_foundation_shared::{
	actor::spatial::{Spatial, SpatialTracker},
	math::EntityAabb,
};

use super::scene_root::ActorSpawnedInGameBehavior;

struct HasSpatialBundle;

impl HasGlobalVirtualTag for HasSpatialBundle {}

pub fn push_spatial_bundle(entity: Entity, aabb: EntityAabb) {
	entity
		.with_tag(GlobalVirtualTag::<HasSpatialBundle>)
		.with_tagged(GlobalTag::<Spatial>, Spatial::new(aabb));
}

pub fn register(bhv: &mut BehaviorRegistry) {
	bhv.register::<ActorSpawnedInGameBehavior>(ActorSpawnedInGameBehavior::new(
		|_bhv, events, scene| {
			let spatial_mgr = &mut *scene.get_mut::<SpatialTracker>();

			query! {
				for (_event in *events; omut spatial in GlobalTag::<Spatial>) + [GlobalVirtualTag::<HasSpatialBundle>] {
					spatial_mgr.register(&mut spatial);
				}
			}
		},
	));
}
