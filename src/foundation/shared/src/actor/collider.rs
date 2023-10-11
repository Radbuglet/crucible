//! Implements a data-structure for iterating through objects partially contained within a given volume.
//!
//! ## Internals
//!
//! To optimize AABB intersection queries, we place every collider into exactly one collider-chunk, which
//! exists on a grid distinct from the regular chunk grid. A collider will always be fully contained
//! in its collider-chunk. To achieve this property, we organize our grid as follows:
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
//! Assuming every entity's AABB is at most half the size of one of these collider-chunks, every entity
//! will be contained in exactly one collider-chunk, making updating this data structure *much* simpler.
//!
//! To query the data structure we simply determine which collider-chunks overlap our AABB and
//! concatenate their entity lists together to form a candidate list. For AABBs less than
//! `HALF_GRID_SIZE`, we will be querying at most 8 chunks.

use bort::{cx, CompMut, Cx, HasGlobalManagedTag, Obj};
use crucible_util::{lang::iter::VolumetricIter, mem::hash::FxHashMap};
use typed_glam::{ext::VecExt, glam::IVec3};

use crate::math::{Aabb3, EntityAabb, EntityVec, EntityVecExt};

// === Grid Math === //

const GRID_SIZE: i32 = 16;
const HALF_GRID_SIZE: i32 = GRID_SIZE / 2;
pub const MAX_SPATIAL_SIZE: f64 = HALF_GRID_SIZE as f64;

fn collider_chunk_for_pos(pos: EntityVec) -> IVec3 {
	pos.block_pos()
		.map(|v| v.div_euclid(HALF_GRID_SIZE))
		.to_glam()
}

fn collider_chunk_for_aabb(aabb: EntityAabb) -> IVec3 {
	debug_assert!(aabb.size.all(|v| v <= MAX_SPATIAL_SIZE));
	collider_chunk_for_pos(aabb.origin)
}

// === ColliderTracker === //

type ColliderMutateCx<'a> = Cx<&'a mut Collider>;
type ColliderQueryCx<'a> = Cx<&'a Collider>;

#[derive(Debug, Default)]
pub struct ColliderManager {
	chunks: FxHashMap<IVec3, Vec<Obj<Collider>>>,
}

impl ColliderManager {
	pub fn new() -> Self {
		Self::default()
	}

	#[clippy::dangerous(direct_collider_access, reason = "spawn an actor instead")]
	pub fn register(&mut self, target: &mut CompMut<Collider>) {
		let chunk = collider_chunk_for_aabb(target.aabb);
		self.register_inner(target, chunk);
	}

	#[clippy::dangerous(direct_collider_access, reason = "despawn an actor instead")]
	pub fn unregister(&mut self, cx: ColliderMutateCx<'_>, target: &mut CompMut<Collider>) {
		let chunk = collider_chunk_for_aabb(target.aabb);
		self.unregister_inner(cx!(cx), target, chunk);
	}

	fn register_inner(&mut self, target_data: &mut CompMut<Collider>, chunk: IVec3) {
		let colliders = self.chunks.entry(chunk).or_default();
		target_data.index = colliders.len();
		colliders.push(CompMut::owner(target_data));
	}

	fn unregister_inner(
		&mut self,
		cx: ColliderMutateCx<'_>,
		target_data: &mut Collider,
		chunk: IVec3,
	) {
		// Remove ourselves from the old chunk
		let hashbrown::hash_map::Entry::Occupied(mut entry) = self.chunks.entry(chunk) else {
			unreachable!()
		};

		let entry_slice = entry.get_mut();

		// Swap-remove ourselves from the vector
		entry_slice.swap_remove(target_data.index);

		// If we displaced something, update its index
		if let Some(displaced) = entry_slice.get(target_data.index) {
			displaced.get_mut_s(cx).index = target_data.index;
		}

		// If the chunk is empty, remove it from the vector
		if entry_slice.is_empty() {
			entry.remove();
		}
	}

	#[clippy::dangerous(direct_collider_access, reason = "update the spatial instead")]
	pub fn update_aabb(
		&mut self,
		cx: ColliderMutateCx<'_>,
		target: &mut CompMut<Collider>,
		aabb: EntityAabb,
	) {
		// Update AABB
		let old_chunk = collider_chunk_for_aabb(target.aabb);
		let new_chunk = collider_chunk_for_aabb(aabb);
		target.aabb = aabb;

		// If we changed chunk, move ourselves into it.
		if old_chunk != new_chunk {
			self.unregister_inner(cx!(cx), target, old_chunk);
			self.register_inner(target, new_chunk);
		}
	}

	pub fn query_in<'a>(
		&'a self,
		cx: ColliderQueryCx<'a>,
		aabb: EntityAabb,
	) -> impl Iterator<Item = Obj<Collider>> + 'a {
		// Determine candidate chunks.
		let candidates = Aabb3::from_corners_max_excl(
			collider_chunk_for_pos(aabb.origin),
			collider_chunk_for_pos(aabb.max_corner()) + IVec3::ONE,
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
		candidates.filter(move |collider| aabb.intersects(collider.get_s(cx!(cx)).aabb))
	}
}

#[derive(Debug)]
pub struct Collider {
	aabb: EntityAabb,
	index: usize,
}

impl HasGlobalManagedTag for Collider {
	type Component = Self;
}

impl Collider {
	pub fn new(aabb: EntityAabb) -> Self {
		Self { aabb, index: 0 }
	}

	pub fn aabb(&self) -> EntityAabb {
		self.aabb
	}
}

// === TrackedCollider === //

#[derive(Debug, Clone)]
pub struct TrackedCollider {
	pub origin_offset: EntityVec,
}

impl HasGlobalManagedTag for TrackedCollider {
	type Component = Self;
}
