use bort::{
	behavior_s, Entity, EventGroup, EventGroupMarkerWith, InitializerBehaviorList,
	OrderedBehaviorList, PartialEntity, VecEventList, VirtualTag,
};
use crucible_foundation_client::{
	engine::{gfx::camera::CameraManager, io::input::InputManager},
	gfx::ui::brush::ImmBrush,
};
use crucible_foundation_shared::{
	actor::manager::ActorSpawned,
	humanoid::{health::HealthUpdated, inventory::InventoryUpdated},
	voxel::data::{BlockMaterialRegistry, WorldVoxelData},
};
use crucible_util::debug::type_id::NamedTypeId;
use typed_glam::glam::Vec2;

// === Event Groups === //

pub type GameBaseEventGroup = EventGroup<dyn GameBaseEventGroupMarker>;

pub trait GameBaseEventGroupMarker:
	EventGroupMarkerWith<VecEventList<ActorSpawned>>
	+ EventGroupMarkerWith<VecEventList<HealthUpdated>>
	+ EventGroupMarkerWith<VecEventList<InventoryUpdated>>
{
}

// === Behaviors === //

// Initialization
behavior_s! {
	pub fn InitGame(
		scene: PartialEntity<'_>,
		engine: Entity,
	)
	as list InitializerBehaviorList<Self>
}

// Updating
behavior_s! {
	pub fn UpdateTickReset(events: &mut GameBaseEventGroup, actor_tag: VirtualTag)
}

behavior_s! {
	pub fn UpdateHandleInputs(
		events: &mut GameBaseEventGroup,
		scene: Entity,
		actor_tag: VirtualTag,
		input: &InputManager,
	)
}

behavior_s! {
	pub fn UpdatePrePhysics(events: &mut GameBaseEventGroup, scene: Entity)
}

behavior_s! {
	pub fn UpdateHandleEarlyEvents(
		events: &mut GameBaseEventGroup,
		engine: Entity,
	)
	as list OrderedBehaviorList<Self, NamedTypeId>
}

behavior_s! {
	pub fn UpdateApplyPhysics(
		actor_tag: VirtualTag,
		world: &WorldVoxelData,
		registry: &BlockMaterialRegistry,
	)
}

behavior_s! {
	pub fn UpdateApplySpatialConstraints(scene: Entity)
}

behavior_s! {
	pub fn UpdatePropagateSpatials(scene: Entity)
}

// Rendering
behavior_s! {
	pub fn RenderProvideCameraBehavior(
		actor_tag: VirtualTag,
		mgr: &mut CameraManager
	)
}

behavior_s! {
	pub fn RenderDrawUiBehavior(brush: &mut ImmBrush<'_>, screen_size: Vec2, scene: Entity)
}
