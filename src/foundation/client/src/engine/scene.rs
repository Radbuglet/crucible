use bort::{delegate, Entity, OwnedEntity};

use super::io::main_loop::MainLoop;

#[derive(Debug, Default)]
pub struct SceneManager {
	current: Option<OwnedEntity>,
	next: Option<OwnedEntity>,
}

impl SceneManager {
	pub fn set_initial(&mut self, scene: OwnedEntity) {
		debug_assert!(self.current.is_none());
		self.current = Some(scene);
	}

	pub fn current(&self) -> Entity {
		self.current
			.as_ref()
			.expect("no initial scene set")
			.entity()
	}

	pub fn set_next_scene(&mut self, next: OwnedEntity) {
		debug_assert!(self.next.is_none());
		self.next = Some(next);
	}

	pub fn swap_scenes(&mut self) -> Option<OwnedEntity> {
		if let Some(next) = self.next.take() {
			self.current.replace(next)
		} else {
			None
		}
	}
}

delegate! {
	pub fn SceneUpdateHandler(&'a self [me: Entity], main_loop: &mut MainLoop)
}

delegate! {
	pub fn SceneRenderHandler(&'a self [me: Entity], viewport: Entity, frame: &mut wgpu::SurfaceTexture)
}
