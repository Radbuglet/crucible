use crucible_core::foundation::lock::*;
use futures::executor::ThreadPool;
use futures::task::SpawnExt;
use std::sync::Arc;
use std::thread::sleep;
use std::time::Duration;

fn main() {
	env_logger::init();

	let rw_mgr = RwLockManager::new();
	let name = Arc::new(RwLock::new(rw_mgr.clone(), "foo".to_string()));
	let age = Arc::new(RwLock::new(rw_mgr.clone(), 42u32));

	// Test 1
	let exec = ThreadPool::new().unwrap();
	let guard = RwGuard::<(&String, &u32)>::lock_now((&name, &age));

	exec.spawn({
		let name = name.clone();
		let age = age.clone();

		async move {
			println!("Stage 1");

			let guard_1 = name.lock_ref_now();
			println!("Name: {}", guard_1.get());
			drop(guard_1);

			println!("Stage 2:");
			let guard_2 = RwGuard::<(&mut String, &u32)>::lock_async((&name, &age)).await;
			drop(guard_2);

			println!("Guard 2 done. Waiting for guard 3...");
			let guard_3 = RwGuard::<(&mut String, &mut u32)>::lock_async((&name, &age)).await;

			println!("Ready!");
			println!("Name: {}", guard_3.get().0);
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
