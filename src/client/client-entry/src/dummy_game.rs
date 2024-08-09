use std::marker::PhantomData;

use bevy_autoken::{
    random_component, spawn_entity, Obj, RandomAccess, RandomEntityExt, SendsEvent,
};
use bevy_ecs::{
    entity::Entity,
    system::{Query, Res},
};
use crucible_math::{Aabb3, Angle3D, Angle3DExt, EntityAabb, EntityVec, WorldVec, WorldVecExt};
use crucible_utils::newtypes::Index;
use crucible_world::{
    collider::{
        AabbHolder, AabbStore, AnyCollision, BlockColliderDescriptor, Collider, ColliderMaterial,
        ColliderMaterialId, VoxelRayCast, WorldCollisions,
    },
    voxel::{
        BlockData, BlockMaterialRegistry, ChunkVoxelData, EntityPointer, KeepInWorld,
        PopulateWorld, WorldChunkCreated, WorldPointer, WorldVoxelData,
    },
};
use main_loop::{InputManager, Viewport, ViewportManager};
use typed_glam::{
    glam::{Vec2, Vec3},
    traits::GlamBacked as _,
};
use winit::{
    event::MouseButton,
    keyboard::KeyCode,
    window::{CursorGrabMode, WindowId},
};

use crate::{
    main_loop::EngineRoot,
    render::{
        helpers::{CameraManager, CameraSettings, CameraViewState, VirtualCamera},
        voxel::MaterialVisualDescriptor,
        GlobalRenderer,
    },
};

// === Components === //

#[derive(Debug)]
pub struct PlayerCameraController {
    pub pos: EntityVec,
    pub facing: Angle3D,
    pub sensitivity: f32,
    pub ctrl_window: WindowId,
    pub has_focus: bool,
}

random_component!(PlayerCameraController);

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

#[allow(clippy::type_complexity)]
pub fn sys_process_camera_controller(
    mut rand: RandomAccess<(
        &InputManager,
        (&mut AabbStore, &mut AabbHolder),
        &mut BlockColliderDescriptor,
        &mut BlockMaterialRegistry,
        &mut ChunkVoxelData,
        &mut PlayerCameraController,
        &mut VirtualCamera,
        &mut WorldVoxelData,
        &mut WorldCollisions,
        &Viewport,
        &ViewportManager,
        SendsEvent<WorldChunkCreated>,
    )>,
    mut query: Query<(
        Entity,
        &Obj<PlayerCameraController>,
        &Obj<VirtualCamera>,
        &Obj<AabbHolder>,
    )>,
    engine_root: Res<EngineRoot>,
) {
    rand.provide(|| {
        let inputs = &*engine_root.0.get::<InputManager>();
        let viewports = &*engine_root.0.get::<ViewportManager>();
        let world = engine_root.0.get::<WorldVoxelData>();
        let mut collisions = engine_root.0.get::<WorldCollisions>();

        let stone = engine_root
            .0
            .get::<BlockMaterialRegistry>()
            .lookup_by_name("crucible:stone")
            .unwrap();

        for (me, &(mut controller), &(mut camera), &aabb) in query.iter_mut() {
            let win_inputs = inputs.window(controller.ctrl_window);
            let viewport = viewports.get_viewport(controller.ctrl_window).unwrap();
            let window = viewport.window();

            // Handle controller focus
            if !controller.has_focus {
                if win_inputs.button(MouseButton::Left).state() {
                    for mode in [CursorGrabMode::Locked, CursorGrabMode::Confined] {
                        if window.set_cursor_grab(mode).is_ok() {
                            break;
                        }
                    }

                    window.set_cursor_visible(false);

                    controller.has_focus = true;
                }
                continue;
            }

            // Handle controller un-focus
            if win_inputs.physical_key(KeyCode::Escape).recently_pressed() {
                let _ = window.set_cursor_grab(CursorGrabMode::None);
                window.set_cursor_visible(true);
                controller.has_focus = false;
                continue;
            }

            // Update facing angle
            let sensitivity = controller.sensitivity;
            controller.facing += Angle3D::from_deg(inputs.mouse_delta().as_vec2() * sensitivity);
            controller.facing = controller.facing.wrap_x().clamp_y_90();

            // Process heading
            let mut heading = Vec3::ZERO;

            if win_inputs.physical_key(KeyCode::KeyW).state() {
                heading += Vec3::Z;
            }

            if win_inputs.physical_key(KeyCode::KeyS).state() {
                heading += Vec3::NEG_Z;
            }

            if win_inputs.physical_key(KeyCode::KeyA).state() {
                heading += Vec3::NEG_X;
            }

            if win_inputs.physical_key(KeyCode::KeyD).state() {
                heading += Vec3::X;
            }

            if win_inputs.physical_key(KeyCode::KeyQ).state() {
                heading += Vec3::NEG_Y;
            }

            if win_inputs.physical_key(KeyCode::KeyE).state() {
                heading += Vec3::Y;
            }

            heading = heading.normalize_or_zero();
            heading = controller.facing.as_matrix().transform_vector3(heading);

            let camera_aabb = Aabb3::from_origin_size(
                camera.state.pos.as_dvec3().cast_glam(),
                EntityVec::splat(0.9),
                EntityVec::splat(0.5),
            );

            aabb.set_aabb(camera_aabb);

            controller.pos += collisions.move_rigid_body(
                camera_aabb,
                (heading * 0.1).as_dvec3().cast_glam(),
                |coll| match coll {
                    AnyCollision::Block(..) => true,
                    AnyCollision::Actor(actor) => actor.entity() != me,
                },
            );

            // Handle interaction
            if win_inputs.button(MouseButton::Right).recently_pressed() {
                for mut isect in VoxelRayCast::new_at(
                    EntityPointer::new(controller.pos),
                    controller.facing.forward().as_dvec3().cast_glam(),
                )
                .step_for(7.)
                {
                    if isect.block.state_or_air(world).is_air() {
                        continue;
                    }

                    let place_at = isect.block.move_to_neighbor(isect.enter_face);

                    if collisions
                        .has_collisions(place_at.pos.full_aabb(), |_| true)
                        .is_none()
                    {
                        place_at.set_state(world, BlockData::new(stone), PopulateWorld);
                    }

                    break;
                }
            }

            if win_inputs.button(MouseButton::Left).recently_pressed() {
                for mut isect in VoxelRayCast::new_at(
                    EntityPointer::new(controller.pos),
                    controller.facing.forward().as_dvec3().cast_glam(),
                )
                .step_for(7.)
                {
                    if isect.block.state_or_air(world).is_air() {
                        continue;
                    }

                    isect.block.set_state(world, BlockData::AIR, KeepInWorld);
                    break;
                }
            }

            // Update camera
            camera.state.pos = controller.pos.as_glam().as_vec3();
            camera.state.facing = controller.facing;
            camera.settings = if win_inputs.physical_key(KeyCode::Space).state() {
                CameraSettings::new_ortho(Vec2::splat(10.), 1., 100.)
            } else {
                CameraSettings::new_persp_deg(90., 0.1, 100.)
            };
        }
    });
}
