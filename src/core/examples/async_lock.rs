use core::foundation::lock::{RwGuard, RwLock, RwLockManager, RwMut, RwRef};
use futures::executor::ThreadPool;
use futures::task::SpawnExt;
use std::sync::Arc;
use std::thread::sleep;
use std::time::Duration;

fn main() {
	env_logger::init();

	let rw_mgr = RwLockManager::new();
	let name = Arc::new(RwLock::new(rw_mgr.clone(), "foo".to_string()));
	let age = Arc::new(RwLock::new(rw_mgr.clone(), 42));

	// Test 1
	let exec = ThreadPool::new().unwrap();
	let guard = RwGuard::lock_now((RwRef(&name), RwMut(&age)));

	exec.spawn({
		let name = name.clone();
		let age = age.clone();

		async move {
			println!("Stage 1");

			let guard_1 = name.lock_ref_now();
			println!("Name: {}", guard_1.get());
			drop(guard_1);

			println!("Stage 2:");
			let guard_2 = RwGuard::lock_async((RwRef(&name), RwRef(&age)))
				.await
				.unwrap();

			println!("Guard 2 done. Waiting for guard 3...");
			let guard_3 = RwGuard::lock_async((RwRef(&name), RwRef(&age)))
				.await
				.unwrap();

			println!("Ready!");
			println!("Name: {}", guard_2.get().0);
			println!("Age: {}", guard_3.get().1);
		}
	})
	.unwrap();

	println!("Holding on...");
	sleep(Duration::from_secs(1));
	println!("Released.");
	drop(guard);

	println!("Letting things finish...");
	sleep(Duration::from_secs(1));
	println!("Goodbye!");
}
