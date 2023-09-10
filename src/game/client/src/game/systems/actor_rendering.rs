use bort::{alias, proc, BehaviorRegistry};
use crucible_foundation_client::{
	engine::{assets::AssetManager, io::gfx::GfxContext},
	gfx::actor::{pipeline::ActorRenderingUniforms, renderer::ActorRenderer},
};

use super::entry::{GameInitRegistry, GameSceneInitBehavior};

// === Behaviors === //

alias! {
	let asset_mgr: AssetManager;
	let gfx: GfxContext;
}

pub fn register(bhv: &mut BehaviorRegistry) {
	let _ = bhv;
}

pub fn push_plugins(pm: &mut GameInitRegistry) {
	pm.register(
		[],
		GameSceneInitBehavior::new(|_bhv, call_cx, scene, engine| {
			proc! {
				as GameSceneInitBehavior[call_cx] do
				(_cx: [], _call_cx: [], mut asset_mgr = engine, ref gfx = engine) {
					scene.add(ActorRenderingUniforms::new(asset_mgr, gfx));
					scene.add(ActorRenderer::default());
				}
			}
		}),
	);
}
