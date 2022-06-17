#![feature(bench_black_box)]

use geode::atomic_ref_cell::ARefCell;
use std::{cell::RefCell, time::Instant};

fn main() {
	let repeats = 50;
	let times = 10_000;

	let regular_cell = RefCell::new(0);

	for _ in 0..repeats {
		let start = Instant::now();

		for _ in 0..times {
			let guard = regular_cell.borrow_mut();
			std::hint::black_box(guard);
		}

		dbg!(start.elapsed());
	}

	println!("===");

	let atomic_cell = ARefCell::new(0);

	for _ in 0..repeats {
		let start = Instant::now();

		for _ in 0..times {
			let _guard = atomic_cell.borrow_mut();
		}

		dbg!(start.elapsed());
	}
}
