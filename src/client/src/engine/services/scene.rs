use geode::prelude::*;
use parking_lot::Mutex;

#[derive(Debug, Clone)]
pub struct SceneUpdateEvent {
	pub engine: Entity,
}

#[derive(Debug)]
pub struct SceneManager {
	scene: Option<Owned<Entity>>,
	next_scene: Mutex<Option<Owned<Entity>>>,
}

impl Default for SceneManager {
	fn default() -> Self {
		Self {
			scene: None,
			next_scene: Mutex::new(Default::default()),
		}
	}
}

impl SceneManager {
	pub fn init_scene(&mut self, scene: Owned<Entity>) {
		assert!(self.scene.is_none());
		self.scene = Some(scene);
	}

	pub fn set_next_scene(&self, scene: Owned<Entity>) -> Option<Owned<Entity>> {
		debug_assert!(
			self.scene.is_some(),
			"Called `set_next_scene` before an initial scene was provided. This was likely unintended."
		);
		self.next_scene.lock().replace(scene)
	}

	pub fn swap_scenes(&mut self) -> Option<Owned<Entity>> {
		debug_assert!(
			self.scene.is_some(),
			"Called `swap_scenes` before an initial scene was provided. This was likely unintended."
		);
		if let Some(next) = self.next_scene.get_mut().take() {
			self.scene.replace(next)
		} else {
			None
		}
	}

	pub fn current_scene(&self) -> Entity {
		self.scene
			.as_ref()
			.expect("Called `current_scene` before an initial scene was provided.")
			.weak_copy()
	}
}
