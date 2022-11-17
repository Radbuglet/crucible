use crucible_core::mem::perfect_map::Phf;
use std::{hint::black_box, time::Instant};

fn main() {
	fastrand::seed(0xDEADBEEF);

	// Collect a bunch of hashes
	let mut elem_hashes = (0..1000).map(|_| fastrand::u32(..)).collect::<Vec<_>>();
	elem_hashes.sort_by(|a, b| a.cmp(&b));

	// Remove duplicates
	{
		let mut prev = None;
		elem_hashes.retain_mut(|a| prev.replace(*a) != Some(*a));
	}

	let start = Instant::now();
	let (phf, slot_to_idx) = Phf::new(elem_hashes.iter().copied());
	dbg!(start.elapsed());

	for (i, &hash) in elem_hashes.iter().enumerate() {
		assert_eq!(slot_to_idx[phf.find_slot(hash)], i);
	}

	// Benchmark
	for _ in 0..100 {
		let hash = elem_hashes[0];
		let start = Instant::now();
		let mut accum = 0;

		for _ in 0..1_000_000 {
			accum += phf.find_slot(hash);
			black_box(());
		}

		dbg!(start.elapsed() / 1_000_000);
		dbg!(accum);
		std::thread::sleep(std::time::Duration::ZERO);
	}
}
