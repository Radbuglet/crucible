use crucible_core::foundation::prelude::*;

fn main() {
	let engine = MultiProvider::<(
		Component<RwLockManager>,
		LazyComponent<RwLock<u8>>,
		LazyComponent<RwLock<u16>>,
		LazyComponent<RwLock<u32>>,
	)>::default();

	engine.init_lock(1u8);
	engine.init_lock(2u16);
	engine.init_lock(3u32);

	let mut guard = RwGuard::<(&u8, &u16, &mut u32)>::lock_now((
		engine.get_lock::<u8>(),
		engine.get_lock::<u16>(),
		engine.get_lock::<u32>(),
	));

	let (a, b, c) = guard.get();
	println!("a: {}", a);
	println!("b: {}", b);
	println!("c: {}", c);

	drop(guard);

	consumer(RwGuard::lock_now(engine.get_many()).get());
}

fn consumer((a, b, c): (&mut u8, &mut u16, &mut u32)) {
	println!("a: {}", a);
	println!("b: {}", b);
	println!("c: {}", c);
}
