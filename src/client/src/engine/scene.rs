#![allow(dead_code)]

use crucible_core::{
	debug::userdata::DebugOpaque,
	ecs::{entity::Entity, provider::Provider},
};

// === SceneManager === //

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

// === Handlers === //

pub type SceneUpdateHandler = DebugOpaque<fn(&Provider, Entity, SceneUpdateEvent)>;
pub type SceneRenderHandler = DebugOpaque<fn(&Provider, Entity, SceneRenderEvent)>;

#[derive(Debug)]
pub struct SceneUpdateEvent {}

#[derive(Debug)]
pub struct SceneRenderEvent<'a> {
	pub frame: &'a mut wgpu::SurfaceTexture,
}
