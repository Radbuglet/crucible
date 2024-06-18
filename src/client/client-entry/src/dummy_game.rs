use std::marker::PhantomData;

use bevy_autoken::{random_component, ObjOwner, RandomAccess, RandomEntityExt};
use bevy_ecs::{
    component::Component,
    entity::Entity,
    system::{Query, Res},
};
use crucible_math::{Angle3D, Angle3DExt};
use main_loop::InputManager;
use typed_glam::glam::Vec3;

use crate::{
    main_loop::EngineRoot,
    render::helpers::{CameraManager, CameraSettings, VirtualCamera},
};

// === Components === //

#[derive(Debug, Component)]
pub struct PlayerCameraController {
    pub sensitivity: f32,
    pub angle: Angle3D,
}

random_component!(PlayerCameraController);

// === Systems === //

pub fn init_engine_root(
    _cx: PhantomData<(
        &mut CameraManager,
        &mut PlayerCameraController,
        &mut VirtualCamera,
    )>,
    engine_root: Entity,
) {
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
