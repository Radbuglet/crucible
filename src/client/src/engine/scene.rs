#![allow(dead_code)]

use crucible_util::debug::userdata::{BoxedUserdata, DebugOpaque};
use geode::prelude::*;

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

// === SceneArch === //

bundle! {
	#[derive(Debug)]
	pub struct SceneBundle {
		pub userdata: BoxedUserdata,
		pub update_handler: SceneUpdateHandler,
		pub render_handler: SceneRenderHandler,
	}
}

impl BuildableArchetypeBundle for SceneBundle {
	fn create_archetype(universe: &Universe) -> ArchetypeHandle<Self> {
		let arch = universe.create_archetype("SceneArch");
		universe.add_archetype_queue_handler(arch.id(), Self::on_destroy);

		arch
	}
}

impl SceneBundle {
	fn on_destroy(universe: &Universe, events: EventQueueIter<EntityDestroyEvent>) {
		let mut guard;
		let mut cx = unpack!(universe => guard & (
			@mut Storage<BoxedUserdata>,
			@mut Storage<SceneUpdateHandler>,
			@mut Storage<SceneRenderHandler>,
		));

		let arch_id = events.arch();
		let mut arch = universe.archetype_by_id(arch_id).lock();

		for (target, _) in events {
			let state = SceneBundle::detach(decompose!(cx), target);
			drop(state);

			arch.despawn(target);
		}
	}
}
