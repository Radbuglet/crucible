use std::marker::PhantomData;

use bevy_autoken::{
    random_component, spawn_entity, Obj, RandomAccess, RandomEntityExt, SendsEvent,
};
use bevy_ecs::{
    component::Component,
    entity::Entity,
    system::{Query, Res},
};
use crucible_math::{Angle3D, Angle3DExt, WorldVec};
use crucible_world::{
    voxel::{
        BlockData, BlockMaterialRegistry, ChunkData, ChunkVoxelData, WorldChunkCreated,
        WorldVoxelData,
    },
    WorldFacade,
};
use main_loop::{InputManager, Viewport, ViewportManager};
use typed_glam::glam::Vec3;
use winit::{keyboard::KeyCode, window::WindowId};

use crate::{
    main_loop::EngineRoot,
    render::{
        helpers::{CameraManager, CameraSettings, VirtualCamera},
        voxel::MaterialVisualDescriptor,
        GlobalRenderer,
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
        &mut GlobalRenderer,
        &mut MaterialVisualDescriptor,
        &mut PlayerCameraController,
        &mut VirtualCamera,
        &mut WorldFacade,
        &mut WorldVoxelData,
        &Viewport,
        &ViewportManager,
        SendsEvent<WorldChunkCreated>,
    )>,
    engine_root: Entity,
) {
    let viewport_mgr = engine_root.get::<ViewportManager>();
    let main_viewport = *viewport_mgr.window_map().keys().next().unwrap();
    let mut renderer = engine_root.get::<GlobalRenderer>();

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
        ctrl_window: main_viewport,
    });

    engine_root.get::<CameraManager>().set_active_camera(camera);

    // Create the basic material
    let stone = renderer.push_to_atlas(
        &image::load_from_memory(include_bytes!("render/embedded_res/stone.png"))
            .unwrap()
            .into_rgba32f(),
    );

    let bricks = renderer.push_to_atlas(
        &image::load_from_memory(include_bytes!("render/embedded_res/bricks.png"))
            .unwrap()
            .into_rgba32f(),
    );

    let mut registry = engine_root.get::<BlockMaterialRegistry>();
    let _air = registry.register("crucible:air", spawn_entity(()));
    let stone = registry.register(
        "crucible:stone",
        spawn_entity(()).with(MaterialVisualDescriptor::cubic_simple(stone)),
    );
    let bricks = registry.register(
        "crucible:bricks",
        spawn_entity(()).with(MaterialVisualDescriptor::cubic_simple(bricks)),
    );

    // Create the root chunk
    let mut world = engine_root.get::<WorldFacade>();

    for x in -10..=10 {
        for y in -10..=10 {
            for z in -10..=10 {
                if fastrand::bool() {
                    continue;
                }

                world.set_block(
                    WorldVec::new(x, y, z),
                    if fastrand::bool() {
                        BlockData::new(bricks)
                    } else {
                        BlockData::new(stone)
                    },
                    |mut cd, _| {
                        cd.initialize_data(ChunkData::AllAir);
                        true
                    },
                );
            }
        }
    }
}

pub fn sys_process_camera_controller(
    mut rand: RandomAccess<(
        &InputManager,
        &mut PlayerCameraController,
        &mut VirtualCamera,
    )>,
    mut query: Query<(&Obj<PlayerCameraController>, &Obj<VirtualCamera>)>,
    engine_root: Res<EngineRoot>,
) {
    rand.provide(|| {
        let inputs = &*engine_root.0.get::<InputManager>();

        for (&(mut controller), &(mut camera)) in query.iter_mut() {
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
