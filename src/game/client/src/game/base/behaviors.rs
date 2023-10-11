use bort::{behavior_s, Entity, InitializerBehaviorList, PartialEntity, VecEventList, VirtualTag};
use crucible_foundation_client::{
	engine::{gfx::camera::CameraManager, io::input::InputManager},
	gfx::ui::brush::ImmBrush,
};
use crucible_foundation_shared::{
	actor::{manager::ActorSpawned, spatial::SpatialMoved},
	voxel::data::{BlockMaterialRegistry, WorldVoxelData},
};
use typed_glam::glam::Vec2;

behavior_s! {
	pub fn GameSceneInitBehavior(
		scene: PartialEntity<'_>,
		engine: Entity,
	)
	as list InitializerBehaviorList<Self>
}

behavior_s! {
	pub fn ActorSpawnedInGameBehavior(
		events: &mut VecEventList<ActorSpawned>,
		engine: Entity,
	)
}

behavior_s! {
	pub fn CameraProviderBehavior(
		actor_tag: VirtualTag,
		mgr: &mut CameraManager
	)
}

behavior_s! {
	pub fn ActorInputBehavior(
		scene: Entity,
		actor_tag: VirtualTag,
		input: &InputManager,
	)
}

behavior_s! {
	pub fn ActorPhysicsResetBehavior(actor_tag: VirtualTag)
}

behavior_s! {
	pub fn ActorPhysicsInfluenceBehavior(actor_tag: VirtualTag)
}

behavior_s! {
	pub fn ActorPhysicsApplyBehavior(
		actor_tag: VirtualTag,
		world: &WorldVoxelData,
		registry: &BlockMaterialRegistry,
		on_spatial_moved: &mut VecEventList<SpatialMoved>,
	)
}

behavior_s! {
	pub fn SpatialUpdateApplyConstraints(
		scene: Entity,
		on_spatial_moved: &VecEventList<SpatialMoved>,
	)
}

behavior_s! {
	pub fn SpatialUpdateApplyUpdates(
		scene: Entity,
		on_spatial_moved: &VecEventList<SpatialMoved>,
	)
}

behavior_s! {
	pub fn UiRenderHudBehavior(brush: &mut ImmBrush<'_>, screen_size: Vec2, scene: Entity)
}
