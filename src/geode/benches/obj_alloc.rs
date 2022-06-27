use criterion::{criterion_group, criterion_main, Criterion};
use geode::prelude::*;

pub fn criterion_benchmark(c: &mut Criterion) {
	c.bench_function("obj_alloc", |b| {
		let session = LocalSessionGuard::new();
		let s = session.handle();

		let _ = Obj::new(s, 4u32);

		b.iter(|| Obj::new(s, 4u32).manually_manage())
	});
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
