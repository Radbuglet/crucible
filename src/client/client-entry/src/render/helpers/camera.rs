use bevy_autoken::{random_component, Obj};
use crucible_math::{Angle3D, Angle3DExt};
use typed_glam::glam::{Mat4, Vec2, Vec3};

// === Math === //

#[derive(Debug, Copy, Clone, Default)]
pub struct CameraViewState {
    pub pos: Vec3,
    pub facing: Angle3D,
}

impl CameraViewState {
    pub fn new(pos: Vec3, facing: Angle3D) -> Self {
        Self { pos, facing }
    }

    pub fn view_xform(self) -> Mat4 {
        self.facing.as_matrix().inverse() * Mat4::from_translation(-self.pos)
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
    pub fn new_persp_rad(fov: f32, near: f32, far: f32) -> Self {
        Self::Perspective { fov, near, far }
    }

    pub fn new_persp_deg(fov: f32, near: f32, far: f32) -> Self {
        Self::new_persp_rad(fov.to_radians(), near, far)
    }

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
    pub fn proj_xform(self, aspect: f32) -> Mat4 {
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
pub struct CameraTransforms {
    pub view: Mat4,
    pub proj: Mat4,
    pub i_view: Mat4,
    pub i_proj: Mat4,
    pub camera: Mat4,
    pub i_camera: Mat4,
}

impl CameraTransforms {
    pub fn new(state: CameraViewState, settings: CameraSettings, aspect: f32) -> Self {
        Self::new_raw(state.view_xform(), settings.proj_xform(aspect))
    }

    pub fn new_raw(view: Mat4, proj: Mat4) -> Self {
        let i_view = view.inverse();
        let i_proj = proj.inverse();
        let camera = proj * view;
        let i_camera = camera.inverse();

        Self {
            view,
            proj,
            i_view,
            i_proj,
            camera,
            i_camera,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CameraSnapshot {
    pub aspect: f32,
    pub state: CameraViewState,
    pub settings: CameraSettings,
    pub xforms: CameraTransforms,
}

impl Default for CameraSnapshot {
    fn default() -> Self {
        Self::new(CameraViewState::default(), CameraSettings::default(), 1.)
    }
}

impl CameraSnapshot {
    pub fn new(state: CameraViewState, settings: CameraSettings, aspect: f32) -> Self {
        Self {
            aspect,
            state,
            settings,
            xforms: CameraTransforms::new(state, settings, aspect),
        }
    }

    pub fn pos(&self) -> Vec3 {
        self.state.pos
    }

    pub fn facing(&self) -> Angle3D {
        self.state.facing
    }

    pub fn view_xform(&self) -> Mat4 {
        self.xforms.view
    }

    pub fn proj_xform(&self) -> Mat4 {
        self.xforms.proj
    }

    pub fn i_view_xform(&self) -> Mat4 {
        self.xforms.i_view
    }

    pub fn i_proj_xform(&self) -> Mat4 {
        self.xforms.i_proj
    }

    pub fn camera_xform(&self) -> Mat4 {
        self.xforms.camera
    }

    pub fn i_camera_xform(&self) -> Mat4 {
        self.xforms.i_camera
    }
}

// === Manager === //

#[derive(Debug, Clone, Default)]
pub struct CameraManager {
    pub active_camera: Option<Obj<VirtualCamera>>,
}

random_component!(CameraManager);

impl CameraManager {
    pub fn set_active_camera(&mut self, camera: Obj<VirtualCamera>) {
        self.active_camera = Some(camera);
    }

    pub fn snapshot(&self, aspect: f32) -> CameraSnapshot {
        self.active_camera
            .filter(|camera| camera.is_alive())
            .map(|v| v.snapshot(aspect))
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Default)]
pub struct VirtualCamera {
    pub state: CameraViewState,
    pub settings: CameraSettings,
}

random_component!(VirtualCamera);

impl VirtualCamera {
    pub fn new(state: CameraViewState, settings: CameraSettings) -> Self {
        Self { state, settings }
    }

    pub fn snapshot(&self, aspect: f32) -> CameraSnapshot {
        CameraSnapshot::new(self.state, self.settings, aspect)
    }
}
