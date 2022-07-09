use std::time::Instant;

use bumpalo::Bump;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use geode::prelude::*;

pub fn criterion_benchmark(c: &mut Criterion) {
	c.bench_function("box_new", |b| {
		b.iter(|| std::mem::forget(black_box(Box::new(4u32))));
	});

	c.bench_function("bump_alloc", |b| {
		let bump = Bump::new();

		b.iter(|| bump.alloc(4u32));
	});

	c.bench_function("obj_alloc", |b| {
		let session = LocalSessionGuard::new();
		let s = session.handle();

		b.iter_custom(|iters| {
			let iters = usize::try_from(iters)
				.expect("That bench request isn't even satisfiable on this platform!");

			s.reserve_slot_capacity(iters as usize);

			let start = Instant::now();

			for _ in 0..iters {
				black_box(Obj::new(s, 4u32)).manually_destruct();
			}

			start.elapsed()
		});
	});

	c.bench_function("obj_deref", |b| {
		let session = LocalSessionGuard::new();
		let s = session.handle();

		let my_obj = Obj::new(s, 3u32).manually_destruct();

		b.iter(|| *my_obj.get(s));
	});

	c.bench_function("regular_deref", |b| {
		let my_box = Box::new(3u32);

		b.iter(|| *my_box);
	});
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
