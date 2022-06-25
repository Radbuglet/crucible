use geode::prelude::*;

use crate::engine::{entry::MainLockKey, scene::SceneUpdateHandler};

pub fn make_game_entry(s: &Session, _main_lock: Lock) -> Owned<Entity> {
	let root = Entity::new(s);

	let update_handler = Obj::new(s, |s: &Session, _me: Entity, engine_root: Entity| {
		let main_lock = engine_root.get_in(s, proxy_key::<MainLockKey>());

		log::info!("Updating scene. Our main lock is {main_lock:?}");
	})
	.to_unsized::<dyn SceneUpdateHandler>();

	root.add(s, update_handler);

	root
}
