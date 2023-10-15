use std::any::TypeId;

use bort::{alias, cx, query, scope, BehaviorRegistry, Cx, GlobalTag, OwnedEntity};
use crucible_foundation_client::{
	engine::{
		gfx::atlas::{AtlasTexture, AtlasTextureGfx},
		io::gfx::GfxContext,
	},
	gfx::voxel::mesh::MaterialVisualDescriptor,
};
use crucible_foundation_shared::{
	actor::spatial::Spatial,
	humanoid::{
		inventory::InventoryData,
		item::{ItemMaterialRegistry, ItemStackBase},
	},
	math::Color4,
	voxel::{
		collision::{CollisionMeta, MaterialColliderDescriptor},
		data::{BlockMaterialId, BlockMaterialRegistry, ChunkVoxelData, WorldVoxelData},
	},
};
use crucible_util::debug::error::ResultExt;

use crate::game::{
	base::{
		behaviors::{InitGame, UpdateHandleEarlyEvents},
		item_data::BaseClientItemDescriptor,
	},
	content::{
		behaviors::GameContentEventGroup,
		player::{LocalPlayer, PlayerInteractEvent},
	},
};

// === Item Descriptors === //

#[derive(Debug, Clone)]
pub struct SimpleBlockItemDescriptor {
	pub placed_block: BlockMaterialId,
}

// === Behaviors === //

alias! {
	let atlas_texture: AtlasTexture;
	let atlas_texture_gfx: AtlasTextureGfx;
	let block_registry: BlockMaterialRegistry;
	let item_registry: ItemMaterialRegistry;
	let world_data: WorldVoxelData;
	let gfx: GfxContext;
}

pub fn register(bhv: &mut BehaviorRegistry) {
	bhv.register_cx(
		[
			TypeId::of::<BlockMaterialRegistry>(),
			TypeId::of::<ItemMaterialRegistry>(),
			TypeId::of::<AtlasTexture>(),
			TypeId::of::<AtlasTextureGfx>(),
		],
		InitGame::new(|_bhv, s, scene, engine| {
			scope!(use let s, inject {
				ref gfx = engine,
				mut atlas_texture = scene,
				mut atlas_texture_gfx = scene,
				mut block_registry = scene,
				mut item_registry = scene,
			});

			let brick_tex = atlas_texture.add(
				&image::load_from_memory(include_bytes!("../res/bricks.png"))
					.unwrap_pretty()
					.into_rgba32f(),
			);

			atlas_texture_gfx.update(gfx, atlas_texture);

			let bricks_block = block_registry.register(
				"crucible:bricks",
				OwnedEntity::new()
					.with_debug_label("bricks block material descriptor")
					.with(MaterialColliderDescriptor::Cubic(CollisionMeta::OPAQUE))
					.with(MaterialVisualDescriptor::cubic_simple(brick_tex)),
			);

			item_registry.register(
				"crucible:bricks",
				OwnedEntity::new()
					.with_debug_label("bricks item material descriptor")
					.with(BaseClientItemDescriptor {
						color: Color4::new(1.0, 1.0, 0.0, 1.0),
					})
					.with(SimpleBlockItemDescriptor {
						placed_block: bricks_block.id,
					}),
			);

			item_registry
				.find_by_name("crucible:stone")
				.unwrap()
				.descriptor
				.insert(SimpleBlockItemDescriptor {
					placed_block: block_registry.find_by_name("crucible:proto").unwrap().id,
				});
		}),
	);

	bhv.register_cx(
		([], []),
		UpdateHandleEarlyEvents::new(|_bhv, s, events, scene| {
			let events: &mut GameContentEventGroup = events.cast_mut();
			scope!(
				use let s,
				access cx: Cx<
					&LocalPlayer,
					&Spatial,
					&InventoryData,
					&SimpleBlockItemDescriptor,
					&MaterialColliderDescriptor,
					&ItemStackBase,
					&mut ChunkVoxelData,
				>,
				inject {
					ref item_registry = scene,
					ref block_registry = scene,
					mut world_data = scene,
				}
			);

			query! {
				for (
					event in events.get::<PlayerInteractEvent>();
					ref spatial in GlobalTag::<Spatial>,
					ref player in GlobalTag::<LocalPlayer>,
					ref inventory in GlobalTag::<InventoryData>,
				) {
					if event.is_right {
						let Some(current_item) = inventory.slot(player.inventory_slot) else {
							continue;
						};

						let current_item = current_item.get_s::<ItemStackBase>(cx!(cx)).material;
						let current_item = item_registry.find_by_id(current_item).descriptor;

						if !current_item.has::<SimpleBlockItemDescriptor>() {
							continue;
						}

						let current_item = current_item.get_s::<SimpleBlockItemDescriptor>(cx!(cx));

						player.place_block_where_looking(
							cx!(cx),
							world_data,
							block_registry,
							spatial,
							7.0,
							current_item.placed_block,
						);
					} else {
						player.break_block_where_looking(
							cx!(cx),
							world_data,
							block_registry,
							spatial,
							7.0,
						);
					}
				}
			}
		}),
	);
}
