use bevy_app::{App, Update};
use bevy_autoken::RandomAppExt;
use bevy_ecs::schedule::IntoSystemConfigs;
use crucible_world::{
    collider::{sys_unregister_dead_aabbs, AabbHolder, AabbStore},
    voxel::{
        sys_add_new_chunks_to_load_queue, sys_add_rcs_to_new_chunks, sys_unlink_dead_chunks,
        sys_unload_dead_chunks, ChunkLoadQueue, ChunkVoxelData, WorldChunkCreated, WorldVoxelData,
    },
};

fn main() {
    color_backtrace::install();
    tracing_subscriber::fmt::init();

    tracing::info!("Hello!");

    let mut app = App::new();

    app.add_random_component::<AabbHolder>();
    app.add_random_component::<AabbStore>();
    app.add_random_component::<ChunkLoadQueue>();
    app.add_random_component::<ChunkVoxelData>();
    app.add_random_component::<WorldVoxelData>();

    app.add_event::<WorldChunkCreated>();

    #[rustfmt::skip]
    app.add_systems(Update, (
        sys_add_rcs_to_new_chunks,
        sys_add_new_chunks_to_load_queue,
        sys_unload_dead_chunks,
        sys_unlink_dead_chunks,
        sys_unregister_dead_aabbs,
    ).chain());

    app.run();
}
