use std::sync::Arc;

use bevy_autoken::RandomAccess;
use bevy_ecs::{
    component::Component,
    entity::Entity,
    event::EventReader,
    query::With,
    system::{Commands, Query},
};

use super::{WorldChunkCreated, WorldVoxelData};

// === Components === /

#[derive(Debug, Default, Component)]
pub struct WorldRc;

#[derive(Debug, Component)]
pub struct ChunkLoadRc(Arc<()>);

impl ChunkLoadRc {
    pub fn keep_alive(&self) -> ChunkKeepAliveHandle {
        ChunkKeepAliveHandle(Arc::clone(&self.0))
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ChunkKeepAliveHandle(Arc<()>);

// === Systems === //

pub fn sys_add_rcs_to_new_chunks(
    mut rand: RandomAccess<&WorldVoxelData>,
    query: Query<(), With<WorldRc>>,
    mut cmd: Commands,
    mut events: EventReader<WorldChunkCreated>,
) {
    rand.provide(|| {
        for &event in events.read() {
            if !query.contains(event.world.entity()) {
                continue;
            }

            let chunk = event.chunk.entity();
            cmd.entity(chunk).insert(ChunkLoadRc(Arc::new(())));
        }
    });
}

pub fn sys_unload_dead_chunks(mut query: Query<(Entity, &ChunkLoadRc)>, mut cmd: Commands) {
    for (chunk, rc) in query.iter_mut() {
        if Arc::strong_count(&rc.0) == 0 {
            cmd.entity(chunk).despawn();
        }
    }
}
