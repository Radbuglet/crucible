use std::ops::Range;

use crate::math::{Aabb3, ChunkVec, Sign};

pub trait Region: Sized {
	fn compare(add: Option<Self>, sub: Option<Self>, iter: impl FnMut(ChunkVec, Sign));
}

fn iter_added(main: Range<i32>, exclude: Range<i32>) -> impl Iterator<Item = i32> {
	(main.start..exclude.start).chain(exclude.end..main.end)
}

impl Region for Aabb3<ChunkVec> {
	fn compare(add: Option<Self>, sub: Option<Self>, mut iter: impl FnMut(ChunkVec, Sign)) {
		// Normalize `add` and `sub` AABBs.
		let zero = Aabb3 {
			origin: ChunkVec::ZERO,
			size: ChunkVec::ZERO,
		};
		let add = add.unwrap_or(zero);
		let sub = sub.unwrap_or(zero);

		// Iter added
		for x in iter_added(
			add.origin.x()..add.max_corner().x(),
			sub.origin.x()..sub.max_corner().x(),
		) {
			for y in iter_added(
				add.origin.y()..add.max_corner().y(),
				sub.origin.y()..sub.max_corner().y(),
			) {
				for z in iter_added(
					add.origin.z()..add.max_corner().z(),
					sub.origin.z()..sub.max_corner().z(),
				) {
					iter(ChunkVec::new(x, y, z), Sign::Positive);
				}
			}
		}
	}
}
