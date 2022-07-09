use std::time::{Duration, Instant};

use geode::prelude::*;

fn main() {
	let repeats = 50;
	let times = 10_000;

	let session = LocalSessionGuard::new();
	let s = session.handle();

	// Standard sessions
	{
		let mut accum = Duration::ZERO;

		for _ in 0..repeats {
			let start = Instant::now();

			for _ in 0..times {
				let obj = Obj::new(s, 1u32).manually_destruct();
				obj.destroy(s);
			}

			accum += dbg!(start.elapsed());
		}

		println!("Average: {:?}", accum / repeats);
		println!("===");
	}

	// TLS sessions
	{
		let mut accum = Duration::ZERO;

		for _ in 0..repeats {
			let start = Instant::now();

			for _ in 0..times {
				let obj = Obj::new(s, 1u32);
				drop(obj);
			}

			accum += dbg!(start.elapsed());
		}

		println!("Average: {:?}", accum / repeats);
		println!("===");
	}
}
