use std::marker::PhantomData;

use bevy_autoken::{
    random_component, spawn_entity, ObjOwner, RandomAccess, RandomEntityExt, SendsEvent,
};
use bevy_ecs::{
    component::Component,
    entity::Entity,
    system::{Query, Res},
};
use crucible_math::{Angle3D, Angle3DExt, WorldVec, WorldVecExt};
use crucible_world::voxel::{
    BlockData, BlockMaterialRegistry, ChunkData, ChunkVoxelData, WorldChunkCreated, WorldVoxelData,
};
use main_loop::InputManager;
use typed_glam::glam::{UVec2, Vec3};

use crate::{
    main_loop::EngineRoot,
    render::{
        helpers::{CameraManager, CameraSettings, VirtualCamera},
        voxel::MaterialVisualDescriptor,
    },
};

// === Components === //

#[derive(Debug, Component)]
pub struct PlayerCameraController {
    pub sensitivity: f32,
    pub angle: Angle3D,
}

random_component!(PlayerCameraController);

// === Systems === //

#[allow(clippy::type_complexity)]
pub fn init_engine_root(
    _cx: PhantomData<(
        &mut CameraManager,
        &mut PlayerCameraController,
        &mut VirtualCamera,
        &mut WorldVoxelData,
        &mut ChunkVoxelData,
        &mut BlockMaterialRegistry,
        &mut MaterialVisualDescriptor,
        SendsEvent<WorldChunkCreated>,
    )>,
    engine_root: Entity,
) {
    // Create the root camera
    let camera = engine_root.insert(VirtualCamera::new_pos_rot(
        Vec3::ZERO,
        Angle3D::new_deg(0., 0.),
        CameraSettings::Perspective {
            fov: 90.,
            near: 0.1,
            far: 100.,
        },
    ));
    engine_root.insert(PlayerCameraController {
        sensitivity: 0.1,
        angle: Angle3D::ZERO,
    });

    engine_root.get::<CameraManager>().set_active_camera(camera);

    // Create the basic material
    let mut registry = engine_root.get::<BlockMaterialRegistry>();
    let _air = registry.register("crucible:air", spawn_entity(()));
    let stone = registry.register(
        "crucible:stone",
        spawn_entity(()).with(MaterialVisualDescriptor::cubic_simple(UVec2::ZERO)),
    );

    // Create the root chunk
    let world = engine_root.get::<WorldVoxelData>();
    let pos = WorldVec::new(0, -1, 0);
    let mut chunk = world.get_or_insert(pos.chunk());
    chunk.initialize_data(ChunkData::AllAir);
    chunk.set_block(
        pos.block(),
        BlockData {
            material: stone,
            variant: 0,
        },
    );
}

pub fn sys_process_camera_controller(
    mut rand: RandomAccess<(
        &InputManager,
        &mut PlayerCameraController,
        &mut VirtualCamera,
    )>,
    mut query: Query<(&ObjOwner<PlayerCameraController>, &ObjOwner<VirtualCamera>)>,
    engine_root: Res<EngineRoot>,
) {
    rand.provide(|| {
        let input_mgr = &*engine_root.0.get::<InputManager>();

        for (&ObjOwner(mut controller), &ObjOwner(mut camera)) in query.iter_mut() {
            // Update facing angle
            let sensitivity = controller.sensitivity;
            controller.angle += Angle3D::from_deg(input_mgr.mouse_delta().as_vec2() * sensitivity);
            controller.angle = controller.angle.wrap_x().clamp_y_90();

            // Update camera
            let pos = camera.view_xform.project_point3(Vec3::ZERO);
            camera.set_pos_rot(pos, controller.angle);
        }
    });
}
