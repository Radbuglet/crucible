use bort::{delegate, scope, BehaviorRegistry, Entity, OwnedEntity};
use crucible_foundation_shared::{humanoid::item::ItemMaterialRegistry, math::Color4};

use super::behaviors::InitGame;

pub fn register(bhv: &mut BehaviorRegistry) {
	bhv.register_cx(
		[],
		InitGame::new(|_bhv, s, scene, _engine| {
			scope!(use let s);

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
				    // 	|bhv, s, actor, scene, is_right_click| {},
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

scope!(pub ItemStackInteractScope);

delegate! {
	pub fn ItemStackInteractHandler(
		bhv: &BehaviorRegistry,
		s: &mut ItemStackInteractScope,
		actor: Entity,
		scene: Entity,
		is_right_click: bool,
	)
}
