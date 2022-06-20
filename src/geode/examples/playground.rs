use geode::{
	entity::Entity,
	obj::{LockToken, ObjCtorExt, Session},
};

fn main() {
	let (mut main_thread_token, main_thread_lock) = LockToken::new("my label");
	let session = Session::new([&mut main_thread_token]);
	let s = &session;

	let my_entity = Entity::new(s);
	my_entity.attach(s, 4u32.as_obj_rw(s, main_thread_lock));
	*my_entity.borrow_mut::<u32>(s) = 5;
	println!("{}", my_entity.borrow::<u32>(s));
}
