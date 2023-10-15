use std::any::TypeId;

use bort::{alias, scope, BehaviorRegistry, OwnedEntity};
use crucible_foundation_client::{
	engine::{
		gfx::atlas::{AtlasTexture, AtlasTextureGfx},
		io::gfx::GfxContext,
	},
	gfx::voxel::mesh::MaterialVisualDescriptor,
};
use crucible_foundation_shared::voxel::{
	collision::{CollisionMeta, MaterialColliderDescriptor},
	data::BlockMaterialRegistry,
};
use crucible_util::debug::error::ResultExt;

use crate::game::base::behaviors::InitGame;

alias! {
	let atlas_texture: AtlasTexture;
	let atlas_texture_gfx: AtlasTextureGfx;
	let block_registry: BlockMaterialRegistry;
	let gfx: GfxContext;
}

pub fn register(bhv: &mut BehaviorRegistry) {
	bhv.register_cx(
		[
			TypeId::of::<BlockMaterialRegistry>(),
			TypeId::of::<AtlasTexture>(),
			TypeId::of::<AtlasTextureGfx>(),
		],
		InitGame::new(|_bhv, s, scene, engine| {
			scope!(use let s, inject {
				ref gfx = engine,
				mut atlas_texture = scene,
				mut atlas_texture_gfx = scene,
				mut block_registry = scene,
			});

			let brick_tex = atlas_texture.add(
				&image::load_from_memory(include_bytes!("../res/bricks.png"))
					.unwrap_pretty()
					.into_rgba32f(),
			);

			// TODO: Defer these using events.
			atlas_texture_gfx.update(gfx, atlas_texture);

			block_registry.register(
				"crucible:bricks",
				OwnedEntity::new()
					.with_debug_label("bricks material descriptor")
					.with(MaterialColliderDescriptor::Cubic(CollisionMeta::OPAQUE))
					.with(MaterialVisualDescriptor::cubic_simple(brick_tex)),
			);
		}),
	);
}
