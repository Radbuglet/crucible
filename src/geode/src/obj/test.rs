use super::{LockToken, Obj, Session};
use std::cell::Cell;

#[test]
fn basic_obj_test() {
	let session = Session::new([]);
	let s = &session;

	let foo = Obj::new(s, 42);
	foo.get(s);
	dbg!(foo.get(s));
	foo.destroy(s);
	dbg!(foo.is_alive_now(s));

	let (mut my_lock_token, my_lock) = LockToken::new(None);
	let bar = Obj::new_in(s, my_lock, Cell::new(0));
	let _ = dbg!(bar.try_get(s));

	let session = Session::new([&mut my_lock_token]);
	let s = &session;

	dbg!(bar.get(s).get());
	bar.get(s).set(42);
	dbg!(bar.get(s).get());
	dbg!(bar.is_alive_now(s));
	bar.destroy(s);
	dbg!(bar.is_alive_now(s));
}
