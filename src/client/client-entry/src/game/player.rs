use bevy_autoken::{random_component, Obj, RandomAccess, RandomEntityExt as _, SendsEvent};
use bevy_ecs::system::{Query, Res};
use crucible_math::{Angle3D, Angle3DExt as _, EntityAabb, EntityVec, WorldVecExt};
use crucible_world::{
    collider::{
        AabbHolder, AabbStore, AnyCollision, BlockColliderDescriptor, VoxelRayCast, WorldCollisions,
    },
    voxel::{
        BlockData, BlockMaterialRegistry, ChunkVoxelData, EntityPointer, KeepInWorld,
        PopulateWorld, WorldChunkCreated, WorldVoxelData,
    },
};
use main_loop::{InputManager, InputManagerWindow, Viewport, ViewportManager};
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
    render::helpers::{CameraSettings, VirtualCamera},
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

impl PlayerCameraController {
    pub fn move_by_colliding(
        mut self: Obj<Self>,
        collisions: &mut WorldCollisions,
        delta: EntityVec,
    ) {
        let me = self.entity();
        let derived_aabb = self.derived_aabb();
        self.pos += collisions.move_rigid_body(derived_aabb, delta, |coll| match coll {
            AnyCollision::Block(..) => true,
            AnyCollision::Actor(actor) => actor.entity() != me,
        });
        self.update_aabb();
    }

    pub fn update_aabb(self: Obj<Self>) {
        self.obj::<AabbHolder>().set_aabb(self.derived_aabb());
    }

    pub fn derived_aabb(&self) -> EntityAabb {
        EntityAabb::from_origin_size(self.pos, EntityVec::splat(0.5), EntityVec::splat(0.5))
    }
}

random_component!(PlayerCameraController);

// === Systems === //

fn get_heading(win_inputs: &InputManagerWindow) -> Vec3 {
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

    heading.normalize_or_zero()
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
    mut query: Query<(&Obj<PlayerCameraController>, &Obj<VirtualCamera>)>,
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

        for (&(mut controller), &(mut camera)) in query.iter_mut() {
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
            let heading = get_heading(&win_inputs);
            let heading = controller.facing.as_matrix().transform_vector3(heading);

            controller.update_aabb();
            controller.move_by_colliding(
                &mut collisions,
                heading.as_dvec3().cast_glam::<EntityVec>() * 0.1,
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
