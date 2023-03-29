use crucible_common::math::{Angle3D, Angle3DExt};
use typed_glam::glam::{Mat4, Vec3};

#[derive(Debug, Default)]
pub struct CameraManager {
	is_locked: bool,
	view: Mat4,
	settings: CameraSettings,
}

impl CameraManager {
	pub fn unset(&mut self) {
		self.is_locked = false;
	}

	pub fn set(&mut self, view: Mat4, settings: CameraSettings) {
		if self.is_locked {
			log::warn!("Provided multiple view transforms in a single frame.");
		}
		self.is_locked = true;
		self.view = view;
		self.settings = settings;
	}

	pub fn set_pos_rot(&mut self, pos: Vec3, angle: Angle3D, settings: CameraSettings) {
		let view = angle.as_matrix().inverse() * Mat4::from_translation(-pos);
		self.set(view, settings);
	}

	pub fn get_view_xform(&self) -> Mat4 {
		if !self.is_locked {
			log::warn!("Called `get_view_xform` on a `CameraManager` before a camera was set.");
		}
		self.view
	}

	pub fn get_settings(&self) -> CameraSettings {
		if !self.is_locked {
			log::warn!("Called `get_settings` on a `CameraManager` before a camera was set.");
		}
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
	Orthogonal {
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
	pub fn proj_matrix(self, aspect: f32) -> Mat4 {
		// FIXME: I have no clue why we have to use left-handed variants to achieve a true right-handed
		//  coordinate system...
		match self {
			Self::Perspective { fov, near, far } => Mat4::perspective_lh(fov, aspect, near, far),
			Self::Orthogonal {
				left,
				right,
				bottom,
				top,
				near,
				far,
			} => Mat4::orthographic_lh(left, right, bottom, top, near, far),
		}
	}
}
