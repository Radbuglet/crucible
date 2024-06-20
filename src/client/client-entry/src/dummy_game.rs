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
use main_loop::{InputManager, ViewportManager};
use typed_glam::glam::{UVec2, Vec3};
use winit::{keyboard::KeyCode, window::WindowId};

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
    pub pos: Vec3,
    pub angle: Angle3D,
    pub sensitivity: f32,
    pub ctrl_window: WindowId,
}

random_component!(PlayerCameraController);

// === Systems === //

#[allow(clippy::type_complexity)]
pub fn init_engine_root(
    _cx: PhantomData<(
        &mut BlockMaterialRegistry,
        &mut CameraManager,
        &mut ChunkVoxelData,
        &mut MaterialVisualDescriptor,
        &mut PlayerCameraController,
        &mut VirtualCamera,
        &mut WorldVoxelData,
        &ViewportManager,
        SendsEvent<WorldChunkCreated>,
    )>,
    engine_root: Entity,
) {
    // Create the root camera
    let camera = engine_root.insert(VirtualCamera::new_pos_rot(
        Vec3::ZERO,
        Angle3D::new_deg(0., 0.),
        CameraSettings::Perspective {
            fov: (90f32).to_radians(),
            near: 0.1,
            far: 100.,
        },
    ));
    engine_root.insert(PlayerCameraController {
        pos: Vec3::ZERO,
        angle: Angle3D::ZERO,
        sensitivity: 0.1,
        ctrl_window: *engine_root
            .get::<ViewportManager>()
            .window_map()
            .keys()
            .next()
            .unwrap(),
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
        let inputs = &*engine_root.0.get::<InputManager>();

        for (&ObjOwner(mut controller), &ObjOwner(mut camera)) in query.iter_mut() {
            // Update facing angle
            let sensitivity = controller.sensitivity;
            controller.angle += Angle3D::from_deg(inputs.mouse_delta().as_vec2() * sensitivity);
            controller.angle = controller.angle.wrap_x().clamp_y_90();

            // Process heading
            let mut heading = Vec3::ZERO;
            let inputs = inputs.window(controller.ctrl_window);

            if inputs.physical_key(KeyCode::KeyW).state() {
                heading += Vec3::Z;
            }

            if inputs.physical_key(KeyCode::KeyS).state() {
                heading += Vec3::NEG_Z;
            }

            if inputs.physical_key(KeyCode::KeyA).state() {
                heading += Vec3::NEG_X;
            }

            if inputs.physical_key(KeyCode::KeyD).state() {
                heading += Vec3::X;
            }

            if inputs.physical_key(KeyCode::KeyQ).state() {
                heading += Vec3::NEG_Y;
            }

            if inputs.physical_key(KeyCode::KeyE).state() {
                heading += Vec3::Y;
            }

            heading = heading.normalize_or_zero();
            heading = controller.angle.as_matrix().transform_vector3(heading);
            controller.pos += heading * 0.1;

            // Update camera
            camera.set_pos_rot(controller.pos, controller.angle);
        }
    });
}
