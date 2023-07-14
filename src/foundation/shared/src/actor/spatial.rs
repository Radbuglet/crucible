//! ## Internals
//!
//! To optimize AABB intersection queries, we place every spatial into exactly one spatial-chunk, which
//! exists on a grid distinct from the regular chunk grid. A spatial will always be fully contained
//! in its spatial-chunk. To achieve this property, we organize our grid as follows:
//!
//! ```text
//!  chunk 1 chunk 2
//!  ~~~~~~~ ~~~~~~~
//! |---!---|---!---|
//!     |---!---|
//!     ~~~~~~~~~
//!     chunk 1.5
//! ```
//!
//! Assuming every entity's AABB is at most half the size of one of these spatial-chunks, every entity
//! will be contained in exactly one spatial-chunk, making updating this data structure *much* simpler.
//!
//! To query the data structure we simply determine which spatial-chunks overlap our AABB and
//! concatenate their entity lists together to form a candidate list. For AABBs less than
//! `HALF_GRID_SIZE`, we will be querying at most 8 chunks.

use bort::prelude::*;
use crucible_util::{lang::iter::VolumetricIter, mem::hash::FxHashMap};
use typed_glam::{ext::VecExt, glam::IVec3};

use crate::math::{Aabb3, EntityAabb, EntityVec, EntityVecExt};

// === Grid Math === //

const GRID_SIZE: i32 = 16;
const HALF_GRID_SIZE: i32 = GRID_SIZE / 2;
pub const MAX_SPATIAL_SIZE: f64 = HALF_GRID_SIZE as f64;

fn spatial_chunk_for_pos(pos: EntityVec) -> IVec3 {
	pos.block_pos()
		.map(|v| v.div_euclid(HALF_GRID_SIZE))
		.to_glam()
}

fn spatial_chunk_for_aabb(aabb: EntityAabb) -> IVec3 {
	debug_assert!(aabb.size.all(|v| v <= MAX_SPATIAL_SIZE));
	spatial_chunk_for_pos(aabb.origin)
}

// === SpatialTracker === //

#[derive(Debug, Default)]
pub struct SpatialTracker {
	chunks: FxHashMap<IVec3, Vec<Obj<Spatial>>>,
}

impl SpatialTracker {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn register(&mut self, target: Obj<Spatial>) {
		let target_data = &mut *target.get_mut();
		let chunk = spatial_chunk_for_aabb(target_data.aabb);
		self.register_inner(target, target_data, chunk);
	}

	pub fn unregister(&mut self, target: Obj<Spatial>) {
		let target_data = &mut *target.get_mut();
		let chunk = spatial_chunk_for_aabb(target_data.aabb);
		self.unregister_inner(target_data, chunk);
	}

	fn register_inner(&mut self, target: Obj<Spatial>, target_data: &mut Spatial, chunk: IVec3) {
		let spatials = self.chunks.entry(chunk).or_insert_with(Default::default);
		target_data.index = spatials.len();
		spatials.push(target);
	}

	fn unregister_inner(&mut self, target_data: &mut Spatial, chunk: IVec3) {
		// Remove ourselves from the old chunk
		let hashbrown::hash_map::Entry::Occupied(mut entry) = self.chunks.entry(chunk) else {
			unreachable!()
		};

		let entry_slice = entry.get_mut();

		// Swap-remove ourselves from the vector
		entry_slice.swap_remove(target_data.index);

		// If we displaced something, update its index
		if let Some(displaced) = entry_slice.get(target_data.index) {
			displaced.get_mut().index = target_data.index;
		}

		// If the chunk is empty, remove it from the vector
		if entry_slice.is_empty() {
			entry.remove();
		}
	}

	pub fn aabb_of(&self, target: Obj<Spatial>) -> EntityAabb {
		target.get().aabb
	}

	pub fn update(&mut self, target: Obj<Spatial>, aabb: EntityAabb) {
		let target_data = &mut *target.get_mut();

		// Update AABB
		let old_chunk = spatial_chunk_for_aabb(target_data.aabb);
		let new_chunk = spatial_chunk_for_aabb(aabb);
		target_data.aabb = aabb;

		// If we changed chunk, move ourselves into it.
		if old_chunk != new_chunk {
			self.unregister_inner(target_data, old_chunk);
			self.register_inner(target, target_data, new_chunk);
		}
	}

	pub fn query_in(&self, aabb: EntityAabb) -> impl Iterator<Item = Obj<Spatial>> + '_ {
		// Determine candidate chunks.
		let candidates = Aabb3::from_corners(
			spatial_chunk_for_pos(aabb.origin),
			spatial_chunk_for_pos(aabb.max_corner()),
		);

		// Determine candidate entities
		let candidates = VolumetricIter::new_inclusive([
			candidates.size.x as _,
			candidates.size.y as _,
			candidates.size.z as _,
		])
		.map(move |[x, y, z]| candidates.origin + IVec3::new(x as i32, y as i32, z as i32))
		.flat_map(|chunk| {
			let candidate_slice = match self.chunks.get(&chunk) {
				Some(chunk) => chunk.as_slice(),
				None => &[],
			};

			candidate_slice.iter().copied()
		});

		// Filter out non-overlapping candidates and yield to the caller
		candidates.filter(move |spatial| aabb.intersects(spatial.get().aabb))
	}
}

#[derive(Debug)]
pub struct Spatial {
	aabb: EntityAabb,
	index: usize,
}

impl Spatial {
	pub fn new(aabb: EntityAabb) -> Self {
		Self { aabb, index: 0 }
	}
}
