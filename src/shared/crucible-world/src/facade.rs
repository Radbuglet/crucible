use std::ops::ControlFlow;

use bevy_autoken::{random_component, Obj, RandomEntityExt};
use bevy_ecs::entity::Entity;
use crucible_math::{EntityAabb, WorldVec, WorldVecExt};

use crate::{
    collider::{
        occluding_volumes_in_block_volume, AabbStore, BlockColliderDescriptor, ColliderMaterial,
    },
    voxel::{
        BlockData, BlockMaterial, BlockMaterialCache, BlockMaterialRegistry, ChunkVoxelData,
        WorldPointer, WorldVoxelData,
    },
};

#[derive(Debug)]
pub enum AnyCollision<'a> {
    Block(&'a mut WorldPointer, EntityAabb, ColliderMaterial),
    Entity(Entity),
}

#[derive(Debug)]
pub struct WorldFacade {
    data: Obj<WorldVoxelData>,
    aabb: Obj<AabbStore>,
    registry: Obj<BlockMaterialRegistry>,
    collider_cache: BlockMaterialCache<BlockColliderDescriptor>,
    pointer: WorldPointer,
}

random_component!(WorldFacade);

impl WorldFacade {
    pub fn new(registry: Obj<BlockMaterialRegistry>, world: Entity) -> Self {
        Self {
            data: world.get::<WorldVoxelData>(),
            aabb: world.get::<AabbStore>(),
            registry,
            collider_cache: BlockMaterialCache::new(registry),
            pointer: WorldPointer::new(WorldVec::ZERO),
        }
    }

    pub fn registry(&self) -> Obj<BlockMaterialRegistry> {
        self.registry
    }

    pub fn lookup(&self, id: &str) -> Option<BlockMaterial> {
        self.registry.lookup_by_name(id)
    }

    pub fn block(&mut self, pos: WorldVec) -> BlockData {
        self.pointer.move_to(pos);
        self.pointer
            .chunk(self.data)
            .map_or(BlockData::AIR, |v| v.block_or_air(pos.block()))
    }

    pub fn set_block(
        &mut self,
        pos: WorldVec,
        data: BlockData,
        if_unloaded: impl FnOnce(Obj<ChunkVoxelData>, &mut WorldPointer) -> bool,
    ) {
        self.pointer.move_to(pos);
        let chunk = self.pointer.chunk_or_insert(self.data);

        if !chunk.is_init() && !if_unloaded(chunk, &mut self.pointer) {
            return;
        }

        chunk.set_block(pos.block(), data);
    }

    pub fn colliders<B>(
        &mut self,
        aabb: EntityAabb,
        mut f: impl FnMut(AnyCollision<'_>) -> ControlFlow<B>,
    ) -> ControlFlow<B> {
        // Handle blocks
        cbit::cbit! {
            for (block, candidate_aabb, mat) in occluding_volumes_in_block_volume(
                self.data,
                &mut self.collider_cache,
                aabb.as_blocks(),
                &mut self.pointer,
            ) {
                if aabb.intersects(candidate_aabb) {
                    f(AnyCollision::Block(block, candidate_aabb, mat))?;
                }
            }
        }

        // Handle entities
        cbit::cbit! {
            for holder in self.aabb.scan(aabb) {
                f(AnyCollision::Entity(holder.entity()))?;
            }
        }

        ControlFlow::Continue(())
    }
}
