use bevy_autoken::{random_component, Obj};
use crucible_math::{Angle3D, Angle3DExt};
use typed_glam::glam::{Mat4, Vec2, Vec3};

// === Components === //

#[derive(Debug, Clone, Default)]
pub struct CameraManager {
    view_xform: Mat4,
    settings: CameraSettings,
    active_camera: Option<Obj<VirtualCamera>>,
}

random_component!(CameraManager);

impl CameraManager {
    pub fn set_active_camera(&mut self, camera: Obj<VirtualCamera>) {
        self.active_camera = Some(camera);
    }

    pub fn recompute(&mut self) {
        let Some(camera) = self.active_camera else {
            return;
        };

        if !camera.is_alive() {
            self.active_camera = None;
            return;
        }

        self.view_xform = camera.view_xform;
        self.settings = camera.settings;
    }

    pub fn get_view_xform(&self) -> Mat4 {
        self.view_xform
    }

    pub fn get_settings(&self) -> CameraSettings {
        self.settings
    }

    pub fn get_proj_xform(&self, aspect: f32) -> Mat4 {
        self.get_settings().proj_matrix(aspect)
    }

    pub fn get_camera_xform(&self, aspect: f32) -> Mat4 {
        self.get_proj_xform(aspect) * self.get_view_xform()
    }
}

#[derive(Debug, Copy, Clone)]
pub enum CameraSettings {
    Perspective {
        fov: f32,
        near: f32,
        far: f32,
    },
    Orthographic {
        left: f32,
        right: f32,
        bottom: f32,
        top: f32,
        near: f32,
        far: f32,
    },
}

impl Default for CameraSettings {
    fn default() -> Self {
        Self::Perspective {
            fov: 70f32.to_radians(),
            near: 0.1,
            far: 100.0,
        }
    }
}

impl CameraSettings {
    pub fn new_ortho(size: Vec2, near: f32, far: f32) -> Self {
        Self::Orthographic {
            left: -size.x,
            right: size.x,
            bottom: -size.y,
            top: size.y,
            near,
            far,
        }
    }

    #[rustfmt::skip]
    pub fn proj_matrix(self, aspect: f32) -> Mat4 {
        // FIXME: I have no clue why we have to use left-handed variants to achieve a true right-handed
        //  coordinate system...
        match self {
            Self::Perspective { fov, near, far } => Mat4::perspective_lh(fov, aspect, near, far),
            Self::Orthographic { left, right, bottom, top, near, far } =>
                Mat4::orthographic_lh(left, right, bottom, top, near, far),
        }
    }
}

#[derive(Debug, Clone)]
pub struct VirtualCamera {
    pub view_xform: Mat4,
    pub settings: CameraSettings,
}

random_component!(VirtualCamera);

impl VirtualCamera {
    pub fn new(view_xform: Mat4, settings: CameraSettings) -> Self {
        Self {
            view_xform,
            settings,
        }
    }

    pub fn new_pos_rot(pos: Vec3, angle: Angle3D, settings: CameraSettings) -> Self {
        let mut camera = Self::new(Mat4::IDENTITY, settings);
        camera.set_pos_rot(pos, angle);
        camera
    }

    pub fn set_pos_rot(&mut self, pos: Vec3, angle: Angle3D) {
        self.view_xform = angle.as_matrix().inverse() * Mat4::from_translation(-pos);
    }
}

// === Systems === //
