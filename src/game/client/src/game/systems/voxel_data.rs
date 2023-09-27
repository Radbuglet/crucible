use bort::{scope, BehaviorRegistry, Cx, OwnedEntity};
use crucible_foundation_client::gfx::voxel::mesh::ChunkVoxelMesh;
use crucible_foundation_shared::voxel::{
	collision::{CollisionMeta, MaterialColliderDescriptor},
	data::{BlockMaterialRegistry, ChunkVoxelData, WorldVoxelData},
	loader::{LoadedChunk, WorldChunkFactory, WorldLoader},
};

use super::entry::{GameInitRegistry, GameSceneInitBehavior};

// === Behaviors === //

pub fn register(bhv: &mut BehaviorRegistry) {
	let _ = bhv;
}

pub fn push_plugins(pm: &mut GameInitRegistry) {
	pm.register(
		[],
		GameSceneInitBehavior::new(|_bhv, s, scene, _engine| {
			scope!(use let s, access cx: Cx<&mut LoadedChunk, &mut ChunkVoxelData>);

			// Create the material registry
			let mut materials = BlockMaterialRegistry::default();
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
		}),
	);
}
