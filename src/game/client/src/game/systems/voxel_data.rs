use bort::{proc, BehaviorRegistry, OwnedEntity};
use crucible_foundation_client::gfx::voxel::mesh::ChunkVoxelMesh;
use crucible_foundation_shared::{
	material::MaterialRegistry,
	voxel::{
		collision::{CollisionMeta, MaterialColliderDescriptor},
		data::{ChunkVoxelData, VoxelDataWriteCx, WorldVoxelData},
		loader::{LoadedChunk, LoaderUpdateCx, WorldChunkFactory, WorldLoader},
	},
};

use super::entry::{GameInitRegistry, GameSceneInitBehavior};

// === Behaviors === //

pub fn register(bhv: &mut BehaviorRegistry) {
	let _ = bhv;
}

pub fn push_plugins(pm: &mut GameInitRegistry) {
	pm.register(
		[],
		GameSceneInitBehavior::new(|_bhv, call_cx, scene, _engine| {
			proc! {
				as GameSceneInitBehavior[call_cx] do
				(cx: [;LoaderUpdateCx, VoxelDataWriteCx], _call_cx: []) {
					// Create the material registry
					let mut materials = MaterialRegistry::default();
					materials.register(
						"crucible:air",
						OwnedEntity::new().with_debug_label("air material descriptor"),
					);

					materials.register(
						"crucible:proto",
						OwnedEntity::new()
							.with_debug_label("prototype material descriptor")
							.with(MaterialColliderDescriptor::Cubic(CollisionMeta::OPAQUE)),
					);

					// Create world
					let world_data = WorldVoxelData::default();
					let world_loader = WorldLoader::new(WorldChunkFactory::new(|_world, pos| {
						// TODO: Also give this an initializer.
						OwnedEntity::new()
							.with_debug_label(format_args!("chunk at {pos:?}"))
							.with(ChunkVoxelData::default().with_default_air_data())
							.with(ChunkVoxelMesh::default())
							.with(LoadedChunk::default())
							.into_obj()
					}));

					// Push services to scene
					scene.add(materials);
					scene.add(world_data);
					scene.add(world_loader);
				}
			}
		}),
	);
}
