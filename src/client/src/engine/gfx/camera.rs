use typed_glam::glam::Mat4;

#[derive(Debug, Default)]
pub struct CameraManager {
	is_locked: bool,
	proj: Mat4,
	fov: f32,
}

impl CameraManager {
	pub fn unlock(&mut self) {
		self.is_locked = false;
	}

	pub fn provide(&mut self, view: Mat4) {
		if self.is_locked {
			log::warn!("Provided multiple view transforms in a single frame.");
		}
		self.is_locked = true;
		self.proj = view;
	}

	pub fn get_view_xform(&self) -> Mat4 {
		if !self.is_locked {
			log::warn!("Called `get_view_xform` on a `CameraManager` before a camera was set.");
		}
		self.proj
	}

	pub fn get_fov(&self) {}
}
