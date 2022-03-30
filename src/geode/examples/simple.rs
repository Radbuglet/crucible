use geode::ecs::world::World;
use geode::exec::obj::{Obj, RwMut, RwRef};
use std::cell::Cell;
use std::mem::replace;

fn main() {
	let root = Obj::new();

	root.add(World::new());
	root.add(SceneManager::new({
		let menu_scene = make_main_scene();
		menu_scene.set_parent(Some(&root));
		menu_scene
	}));

	let sm = root.borrow_ref::<SceneManager>();
	sm.current_scene().fire(UpdateEvent);

	let mut sm = RwRef::upgrade(sm);
	sm.swap_scene();

	let sm = RwMut::downgrade(sm);
	sm.current_scene().fire(UpdateEvent);
}

fn make_main_scene() -> Obj {
	let scene = Obj::new();
	scene.add_event_handler(|_: UpdateEvent, sm: RwRef<SceneManager>| {
		println!("Wow, an update in main!");

		sm.set_next_scene({
			let play_scene = make_play_scene();
			play_scene.set_parent(Some(RwRef::obj(&sm)));
			play_scene
		});
	});
	scene
}

fn make_play_scene() -> Obj {
	let scene = Obj::new();
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
			next_scene: Cell::new(None),
			current_scene: initial_scene,
		}
	}

	pub fn current_scene(&self) -> &Obj {
		&self.current_scene
	}

	pub fn set_next_scene(&self, scene: Obj) {
		self.next_scene.replace(Some(scene));
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
