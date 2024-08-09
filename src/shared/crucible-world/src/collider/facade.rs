use std::ops::ControlFlow;

use bevy_autoken::{random_component, Obj, RandomEntityExt};
use bevy_ecs::entity::Entity;
use crucible_math::{EntityAabb, EntityVec};

use crate::voxel::{BlockMaterialCache, VoxelPointer, WorldPointer, WorldVoxelData};

use super::{
    move_rigid_body_voxels, occluding_volumes_in_entity_volume, AabbHolder, AabbStore,
    BlockColliderDescriptor, ColliderMaterial,
};

#[derive(Debug)]
pub struct WorldCollisions {
    aabbs: Obj<AabbStore>,
    voxels: Obj<WorldVoxelData>,
    cache: BlockMaterialCache<BlockColliderDescriptor>,
}

random_component!(WorldCollisions);

impl WorldCollisions {
    pub fn new(me: Entity) -> Self {
        Self {
            aabbs: me.get(),
            voxels: me.get(),
            cache: BlockMaterialCache::new(me.get()),
        }
    }

    pub fn collisions<B>(
        &mut self,
        aabb: EntityAabb,
        mut f: impl FnMut(AnyCollision) -> ControlFlow<B>,
    ) -> ControlFlow<B> {
        // Check blocks
        cbit::cbit!(for (ptr, _aabb, mat) in occluding_volumes_in_entity_volume(
            self.voxels,
            &mut self.cache,
            aabb,
            &mut VoxelPointer::default(),
        ) {
            f(AnyCollision::Block(*ptr, mat))?;
        });

        // Check entities
        cbit::cbit!(for actor in self.aabbs.scan(aabb) {
            f(AnyCollision::Actor(actor))?;
        });

        ControlFlow::Continue(())
    }

    pub fn has_collisions(
        &mut self,
        aabb: EntityAabb,
        mut filter: impl FnMut(AnyCollision) -> bool,
    ) -> Option<AnyCollision> {
        cbit::cbit!(for c in self.collisions(aabb) {
            if filter(c) {
                return Some(c);
            }
        });

        None
    }

    pub fn move_rigid_body(
        &mut self,
        aabb: EntityAabb,
        delta: EntityVec,
        mut filter: impl FnMut(AnyCollision) -> bool,
    ) -> EntityVec {
        move_rigid_body_voxels(self.voxels, &mut self.cache, aabb, delta, |ptr, mat| {
            filter(AnyCollision::Block(ptr, mat))
        })
    }
}

#[derive(Debug, Copy, Clone)]
pub enum AnyCollision {
    Block(WorldPointer, ColliderMaterial),
    Actor(Obj<AabbHolder>),
}

impl AnyCollision {
    pub fn material(&self) -> ColliderMaterial {
        match self {
            AnyCollision::Block(_, mat) => *mat,
            AnyCollision::Actor(entity) => entity.material(),
        }
    }
}
