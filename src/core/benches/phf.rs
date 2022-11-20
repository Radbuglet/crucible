use criterion::{criterion_group, criterion_main, Criterion};
use crucible_core::mem::perfect_map::Phf;
use nohash_hasher::BuildNoHashHasher;

use std::{collections::HashSet, hint::black_box};

fn random_hashes(count: usize) -> Vec<u32> {
	// Collect a bunch of hashes
	let mut elem_hashes = (0..count).map(|_| fastrand::u32(..)).collect::<Vec<_>>();
	elem_hashes.sort_by(|a, b| a.cmp(&b));

	// Remove duplicates
	{
		let mut prev = None;
		elem_hashes.retain_mut(|a| prev.replace(*a) != Some(*a));
	}

	elem_hashes
}

fn criterion_benchmark(c: &mut Criterion) {
	c.bench_function("phf_create", |c| {
		let elem_hashes = random_hashes(25);
		c.iter(|| Phf::new(black_box(&elem_hashes).iter().copied()));
	});

	c.bench_function("phf_access", |c| {
		let hashes = random_hashes(25);
		let (mut phf, slot_to_idx) = Phf::new(hashes.iter().copied());

		let mut i = 0;

		c.iter(|| {
			i += 1;
			if i >= hashes.len() {
				i = 0;
			}

			black_box(&mut phf);
			slot_to_idx[phf.find_slot(hashes[i], slot_to_idx.len())]
		});
	});

	c.bench_function("set_create", |c| {
		let elem_hashes = random_hashes(25);

		c.iter(|| {
			black_box(&elem_hashes)
				.iter()
				.copied()
				.collect::<HashSet<_, BuildNoHashHasher<u32>>>()
		});
	});

	c.bench_function("set_access", |c| {
		let hashes = random_hashes(25);
		let mut elem_hashes = hashes
			.iter()
			.copied()
			.collect::<HashSet<_, BuildNoHashHasher<u32>>>();

		let mut i = 0;

		c.iter(|| {
			i += 1;
			if i >= hashes.len() {
				i = 0;
			}

			black_box(&mut elem_hashes);
			elem_hashes.contains(&hashes[i])
		});
	});
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
