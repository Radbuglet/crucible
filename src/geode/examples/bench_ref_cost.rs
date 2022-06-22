#![feature(bench_black_box)]

use geode::{Obj, Session};
use std::{
	hint::black_box,
	time::{Duration, Instant},
};

fn main() {
	let repeats = 100;
	let times = 10_000;

	// System allocator
	{
		let foo = Box::new(1);

		// Setup & run test
		let mut accum = Duration::ZERO;
		let mut val = 0;

		for _ in 0..repeats {
			let start = Instant::now();

			for _ in 0..times {
				val += black_box(*foo);
			}

			accum += dbg!(start.elapsed());
		}

		assert_eq!(repeats * times, val);
		println!("Computed: {val}");
		println!("Average: {:?}", accum / repeats);
		println!("===");
	}

	// Geode allocator
	{
		// Setup dependencies
		let session = Session::new([]);
		let s = &session;

		let foo = Obj::new(s, 1);

		// Setup & run test
		let mut accum = Duration::ZERO;
		let mut val = 0;

		for _ in 0..repeats {
			let start = Instant::now();

			for _ in 0..times {
				// Yeah, we really spammed `block_boxes` here...
				val += black_box(*black_box(black_box(foo).get(black_box(s))));
			}

			accum += dbg!(start.elapsed());
		}

		assert_eq!(repeats * times, val);
		println!("Computed: {val}");
		println!("Average: {:?}", accum / repeats);
		println!("===");
	}
}
