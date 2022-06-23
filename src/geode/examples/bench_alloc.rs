#![feature(bench_black_box)]

use bumpalo::Bump;
use geode::{ObjCtorExt, Session};
use std::time::{Duration, Instant};

fn main() {
	let repeats = 50;
	let times = 10_000;

	// Standard allocator
	{
		let mut accum = Duration::ZERO;

		for _ in 0..repeats {
			let start = Instant::now();

			for _ in 0..times {
				let b = Box::new(1);
				std::mem::forget(std::hint::black_box(b));
			}

			accum += dbg!(start.elapsed());
		}

		println!("Average: {:?}", accum / repeats);
		println!("===");
	}

	// Geode allocator
	{
		let mut accum = Duration::ZERO;
		let session = Session::new([]);

		// Grow the session free list by allocating a bunch of entities and promptly deleting them.
		let mut objects = Vec::new();
		for _ in 0..repeats {
			for _ in 0..times {
				objects.push(1.box_obj(&session));
			}
		}

		for obj in objects {
			obj.destroy(&session);
		}

		// Run the actual test
		for _ in 0..repeats {
			let start = Instant::now();

			for _ in 0..times {
				let _b = 1.box_obj(&session);
			}

			accum += dbg!(start.elapsed());
		}

		println!("Average: {:?}", accum / repeats);
		println!("===");
	}

	// Bump allocator
	{
		let mut accum = Duration::ZERO;
		let session = Bump::new();

		for _ in 0..repeats {
			let start = Instant::now();

			for _ in 0..times {
				let _b = session.alloc(1);
			}

			accum += dbg!(start.elapsed());
		}

		println!("Average: {:?}", accum / repeats);
		println!("===");
	}
}
