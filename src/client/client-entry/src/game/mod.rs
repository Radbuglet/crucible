use std::marker::PhantomData;

use bevy_autoken::{spawn_entity, RandomEntityExt, SendsEvent};
use bevy_ecs::entity::Entity;
use crucible_math::{Angle3D, EntityAabb, EntityVec, WorldVec};
use crucible_utils::newtypes::Index;
use crucible_world::{
    collider::{
        AabbHolder, AabbStore, BlockColliderDescriptor, Collider, ColliderMaterial,
        ColliderMaterialId,
    },
    voxel::{
        BlockData, BlockMaterialRegistry, ChunkVoxelData, PopulateWorld, WorldChunkCreated,
        WorldPointer, WorldVoxelData,
    },
};
use main_loop::{Viewport, ViewportManager};

use crate::render::{
    helpers::{CameraManager, CameraSettings, CameraViewState, VirtualCamera},
    voxel::MaterialVisualDescriptor,
    GlobalRenderer,
};

use self::player::PlayerCameraController;

pub mod player;

// === Systems === //

#[allow(clippy::type_complexity)]
pub fn init_engine_root(
    _cx: PhantomData<(
        (&mut AabbStore, &mut AabbHolder),
        &mut BlockColliderDescriptor,
        &mut BlockMaterialRegistry,
        &mut CameraManager,
        &mut ChunkVoxelData,
        &mut GlobalRenderer,
        &mut MaterialVisualDescriptor,
        &mut PlayerCameraController,
        &mut VirtualCamera,
        &mut WorldVoxelData,
        (&Viewport, &ViewportManager),
        SendsEvent<WorldChunkCreated>,
    )>,
    engine_root: Entity,
) {
    let viewport_mgr = engine_root.get::<ViewportManager>();
    let main_viewport = *viewport_mgr.window_map().keys().next().unwrap();
    let mut renderer = engine_root.get::<GlobalRenderer>();

    // Create the root camera
    let camera = engine_root.insert(VirtualCamera::new(
        CameraViewState::default(),
        CameraSettings::new_persp_deg(90f32, 0.1, 100.),
    ));
    engine_root.insert(PlayerCameraController {
        pos: EntityVec::ZERO,
        facing: Angle3D::ZERO,
        sensitivity: 0.1,
        ctrl_window: main_viewport,
        has_focus: false,
    });
    engine_root.insert(AabbHolder::new(
        EntityAabb::ZERO,
        ColliderMaterial {
            id: ColliderMaterialId::from_usize(0),
            meta: 0,
        },
    ));
    engine_root.get::<AabbStore>().register(engine_root.get());

    engine_root.get::<CameraManager>().set_active_camera(camera);

    // Create the basic material
    let stone = renderer.push_to_atlas(
        &image::load_from_memory(include_bytes!("res/stone.png"))
            .unwrap()
            .into_rgba32f(),
    );

    let bricks = renderer.push_to_atlas(
        &image::load_from_memory(include_bytes!("res/bricks.png"))
            .unwrap()
            .into_rgba32f(),
    );

    let mut registry = engine_root.get::<BlockMaterialRegistry>();
    let _air = registry.register("crucible:air", spawn_entity(()));

    let solid_mat = ColliderMaterial {
        id: ColliderMaterialId::from_usize(0),
        meta: 0,
    };
    let stone = registry.register(
        "crucible:stone",
        spawn_entity(())
            .with(MaterialVisualDescriptor::cubic_simple(stone))
            .with(BlockColliderDescriptor(Collider::Opaque(solid_mat))),
    );
    let _bricks = registry.register(
        "crucible:bricks",
        spawn_entity(())
            .with(MaterialVisualDescriptor::cubic_simple(bricks))
            .with(BlockColliderDescriptor(Collider::Opaque(solid_mat))),
    );

    // Create the root chunk
    let mut pointer = WorldPointer::default();
    let world = engine_root.get::<WorldVoxelData>();

    for x in -16..=16 {
        for z in -16..=16 {
            pointer.move_to(WorldVec::new(x, -5, z)).set_state(
                world,
                BlockData::new(stone),
                PopulateWorld,
            );
        }
    }
}
