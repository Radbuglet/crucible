use crucible_core::ecs::core::Entity;

#[derive(Debug, Default)]
pub struct SceneManager {
	current: Option<Entity>,
	next: Option<Entity>,
}

impl SceneManager {
	pub fn set_initial(&mut self, scene: Entity) {
		debug_assert!(self.current.is_none());
		self.current = Some(scene);
	}

	pub fn current(&self) -> Entity {
		self.current.expect("no initial scene set")
	}

	pub fn set_next_scene(&mut self, next: Entity) {
		debug_assert!(self.next.is_none());
		self.next = Some(next);
	}
}
