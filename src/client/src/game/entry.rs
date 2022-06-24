use geode::prelude::*;

pub fn make_game_entry(s: &Session, _main_lock: Lock) -> Owned<Entity> {
	let root = Entity::new(s);
	// root.add(s, components);

	root
}
