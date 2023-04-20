use bort::{Entity, OwnedEntity};

use crate::engine::scene::{SceneRenderHandler, SceneUpdateHandler};

pub fn make_game_scene(_engine: Entity, _main_viewport: Entity) -> OwnedEntity {
	OwnedEntity::new()
		.with_debug_label("game scene")
		.with(SceneUpdateHandler::new(|_me| {}))
		.with(SceneRenderHandler::new(|_me, _frame| {}))
}
