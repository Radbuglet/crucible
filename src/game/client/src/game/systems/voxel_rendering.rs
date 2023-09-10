use bort::{alias, proc, BehaviorRegistry};
use crucible_foundation_client::{
	engine::{
		assets::AssetManager,
		gfx::atlas::{AtlasTexture, AtlasTextureGfx},
		io::gfx::GfxContext,
	},
	gfx::voxel::{
		mesh::{MaterialVisualDescriptor, WorldVoxelMesh},
		pipeline::VoxelUniforms,
	},
};
use crucible_foundation_shared::material::MaterialRegistry;
use crucible_util::debug::{error::ResultExt, type_id::NamedTypeId};
use typed_glam::glam::UVec2;

use super::entry::{GameInitRegistry, GameSceneInitBehavior};

// === Behaviors === //

alias! {
	let asset_mgr: AssetManager;
	let gfx: GfxContext;
	let materials: MaterialRegistry;
}

pub fn register(bhv: &mut BehaviorRegistry) {
	let _ = bhv;
}

pub fn push_plugins(pm: &mut GameInitRegistry) {
	pm.register(
		[NamedTypeId::of::<MaterialRegistry>()],
		GameSceneInitBehavior::new(|_bhv, call_cx, scene, engine| {
			proc! {
				as GameSceneInitBehavior[call_cx] do
				(_cx: [], _call_cx: [], mut asset_mgr = engine, ref gfx = engine, ref materials = scene) {
					// Create atlas
					let mut atlas = AtlasTexture::new(UVec2::new(16, 16), UVec2::new(2, 2), 4);
					let mut atlas_gfx =
						AtlasTextureGfx::new(gfx, &atlas, Some("block texture atlas"));

					// Load builtin textures into atlas
					let proto_tex = atlas.add(
						&image::load_from_memory(include_bytes!("../res/proto_1.png"))
							.unwrap_pretty()
							.into_rgba32f()
					);
					atlas_gfx.update(gfx, &atlas);

					// Give textures to builtin materials
					materials.find_by_name("crucible:proto")
						.unwrap()
						.descriptor
						.insert(MaterialVisualDescriptor::cubic_simple(proto_tex));

					// Register services
					scene.add(VoxelUniforms::new(asset_mgr, gfx, &atlas_gfx.view));
					scene.add(WorldVoxelMesh::default());
					scene.add(atlas);
					scene.add(atlas_gfx);
				}
			}
		}),
	);
}
