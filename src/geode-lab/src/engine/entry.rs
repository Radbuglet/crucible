use crate::util::obj::prelude::*;
use std::{cell::Cell, collections::VecDeque, fmt::Display};

pub fn start_engine() {
	let (mut lt_root, l_root) = LockToken::new();
	let s = Session::acquire([&mut lt_root]);

	let console = Console::default().as_obj_rw(&s, l_root);
	let scene_mgr = SceneManager::default().as_obj_locked(&s, l_root);

	let mut console_p = console.borrow_mut(&s);
	console_p.info("Whee");
	console_p.info("Woo");
}

#[derive(Default)]
pub struct SceneManager {
	current_scene: Option<Obj<()>>,
	next_scene: Cell<Option<Obj<()>>>,
}

impl SceneManager {
	pub fn current_scene(&self) -> Obj<()> {
		self.current_scene.unwrap()
	}

	pub fn set_next_scene(&self, scene: Obj<()>) {
		self.next_scene.set(Some(scene));
	}
}

#[derive(Debug, Default)]
pub struct Console {
	messages: VecDeque<String>,
}

impl Console {
	pub fn info<F: Display>(&mut self, message: F) {
		self.messages.push_back(message.to_string());
	}
}
