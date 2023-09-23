use bort::{
	call_cx, delegate, proc_collection, BehaviorProvider, BehaviorRegistry, Entity, OwnedEntity,
};
use crucible_foundation_shared::{humanoid::item::ItemMaterialRegistry, math::Color4};

use super::entry::{GameInitRegistry, GameSceneInitBehavior};

// === Behaviors === //

pub fn register(bhv: &mut BehaviorRegistry) {
	let _ = bhv;
}

pub fn push_plugins(pm: &mut GameInitRegistry) {
	pm.register(
		[],
		GameSceneInitBehavior::new(|_bhv, _call_cx, scene, _engine| {
			let mut registry = ItemMaterialRegistry::default();
			registry.register(
				"crucible:air",
				OwnedEntity::new().with_debug_label("air item descriptor"),
			);
			registry.register(
				"crucible:stone",
				OwnedEntity::new()
					.with_debug_label("stone material descriptor")
					.with(BaseClientItemDescriptor {
						color: Color4::new(1.0, 0.0, 0.0, 1.0),
					}), // .with(ItemStackInteractHandler::new(
				    // 	|bhv, call_cx, actor, scene, is_right_click| {},
				    // )),
			);

			scene.add(registry);
		}),
	);
}

// === Descriptors === //

#[derive(Debug, Clone)]
pub struct BaseClientItemDescriptor {
	pub color: Color4,
}

delegate! {
	pub fn ItemStackInteractHandler(
		bhv: BehaviorProvider<'_>,
		call_cx: &mut call_cx![Self],
		actor: Entity,
		scene: Entity,
		is_right_click: bool,
	)
	as deriving proc_collection
}
