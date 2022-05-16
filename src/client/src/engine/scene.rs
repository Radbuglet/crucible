use geode::prelude::*;

#[derive(Debug)]
pub struct SceneManager {
	scene: Option<Obj>,
	next_scene: antidote::Mutex<Option<Obj>>,
}

impl Default for SceneManager {
	fn default() -> Self {
		Self {
			scene: None,
			next_scene: antidote::Mutex::new(Default::default()),
		}
	}
}

impl SceneManager {
	pub fn init_scene(&mut self, scene: Obj) {
		assert!(self.scene.is_none());
		self.scene = Some(scene);
	}

	pub fn set_next_scene(&self, scene: Obj) -> Option<Obj> {
		debug_assert!(
			self.scene.is_some(),
			"Called `set_next_scene` before an initial scene was provided. This was likely unintended."
		);
		self.next_scene.lock().replace(scene)
	}

	pub fn swap_scenes(&mut self) -> Option<Obj> {
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

	pub fn current_scene(&self) -> &Obj {
		self.scene
			.as_ref()
			.expect("Called `current_scene` before an initial scene was provided.")
	}
}
