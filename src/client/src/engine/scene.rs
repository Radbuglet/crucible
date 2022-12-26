#![allow(dead_code)]

use crucible_core::{
	debug::userdata::{BoxedUserdata, DebugOpaque},
	ecs::{
		entity::{ArchetypeId, Entity},
		event::{EntityDestroyEvent, EventQueueIter},
		provider::{unpack, Provider},
		storage::Storage,
		universe::{ArchetypeHandle, Universe, UniverseResource},
	},
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

// === SceneArch === //

#[derive(Debug)]
pub struct SceneArch(ArchetypeHandle);

impl UniverseResource for SceneArch {
	fn create(universe: &Universe) -> Self {
		let arch = universe.create_archetype("SceneArch");
		universe.add_archetype_handler(arch.id(), Self::on_destroy);

		Self(arch)
	}
}

impl SceneArch {
	pub fn id(&self) -> ArchetypeId {
		self.0.id()
	}

	pub fn spawn(
		&self,
		cx: &Provider,
		scene_userdata: BoxedUserdata,
		update_handler: SceneUpdateHandler,
		render_handler: SceneRenderHandler,
	) -> Entity {
		unpack!(cx => {
			universe: ~ref Universe,
			scene_userdatas: @mut Storage<BoxedUserdata>,
			update_handlers: @mut Storage<SceneUpdateHandler>,
			render_handlers: @mut Storage<SceneRenderHandler>,
		});

		let scene = universe.archetype(self.id()).spawn("scene");
		scene_userdatas.add(scene, scene_userdata);
		update_handlers.add(scene, update_handler);
		render_handlers.add(scene, render_handler);

		scene
	}

	fn on_destroy(cx: &Provider, universe: &Universe, events: EventQueueIter<EntityDestroyEvent>) {
		unpack!(cx => {
			scene_userdatas: @mut Storage<BoxedUserdata>,
			update_handlers: @mut Storage<SceneUpdateHandler>,
			render_handlers: @mut Storage<SceneRenderHandler>,
		});

		let arch_id = events.arch();
		let mut arch = universe.archetype(arch_id);

		for (target, _) in events {
			scene_userdatas.remove(target);
			update_handlers.remove(target);
			render_handlers.remove(target);
			arch.despawn(target);
		}
	}
}
