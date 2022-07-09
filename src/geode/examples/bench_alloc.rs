#![feature(bench_black_box)]

use bumpalo::Bump;
use geode::prelude::*;
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
				let b = Box::new(1u64);
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
		let session = LocalSessionGuard::new();
		let s = session.handle();

		// Grow the session free list by allocating a bunch of entities and promptly deleting them.
		let mut objects = Vec::new();
		for _ in 0..repeats {
			for _ in 0..times {
				objects.push(1u64.box_obj(s));
			}
		}

		drop(objects);

		// Run the actual test
		for _ in 0..repeats {
			let start = Instant::now();

			for _ in 0..times {
				let _b = 1.box_obj(s).manually_destruct();
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
