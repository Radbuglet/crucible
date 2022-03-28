use geode::ecs::world::World;
use geode::exec::obj::Obj;
use std::cell::Cell;
use std::mem::replace;

fn main() {
	let mut root = Obj::new();

	root.add(World::new());
	root.add(SceneManager::new(make_main_scene()));

	let mut sm = root.borrow_mut::<SceneManager>();
	sm.current_scene().fire(UpdateEvent);
	sm.set_next_scene(make_play_scene());
	sm.swap_scene();
	sm.current_scene().fire(UpdateEvent);
}

fn make_main_scene() -> Obj {
	let mut scene = Obj::new();
	scene.add_event_handler(|_: UpdateEvent| {
		println!("Wow, an update in main!");
	});
	scene
}

fn make_play_scene() -> Obj {
	let mut scene = Obj::new();
	scene.add_event_handler(|_: UpdateEvent| {
		println!("Wow, an update in play!");
	});
	scene
}

struct SceneManager {
	next_scene: Cell<Option<Obj>>,
	current_scene: Obj,
}

impl SceneManager {
	pub fn new(initial_scene: Obj) -> Self {
		Self {
			next_scene: Default::default(),
			current_scene: initial_scene,
		}
	}

	pub fn current_scene(&self) -> &Obj {
		&self.current_scene
	}

	pub fn set_next_scene(&self, scene: Obj) {
		let replaced = self.next_scene.replace(Some(scene));
		debug_assert!(replaced.is_none());
	}

	pub fn swap_scene(&mut self) -> Option<Obj> {
		if let Some(next_scene) = self.next_scene.take() {
			Some(replace(&mut self.current_scene, next_scene))
		} else {
			None
		}
	}
}

struct UpdateEvent;
