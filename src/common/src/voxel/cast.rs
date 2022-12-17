use crucible_core::{
	ecs::storage::CelledStorageView,
	lang::iter::{ContextualIter, WithContext},
	mem::c_enum::CEnum,
};
use smallvec::SmallVec;

use crate::voxel::math::{Axis3, BlockFace, EntityVecExt, Line3, Sign, Vec3Ext, WorldVecExt};

use super::{
	data::{BlockLocation, EntityLocation, VoxelChunkData, VoxelWorldData},
	math::EntityVec,
};

// === RayCast === //

#[derive(Debug, Clone)]
pub struct RayCast {
	loc: EntityLocation,
	dir: EntityVec,
	dist: f64,
}

impl RayCast {
	pub fn new_at(loc: EntityLocation, dir: EntityVec) -> Self {
		let (dir, dist) = {
			let dir_len_recip = dir.length_recip();

			if dir_len_recip.is_finite() && dir_len_recip > 0.0 {
				(dir * dir_len_recip, 1.)
			} else {
				(EntityVec::ZERO, f64::INFINITY)
			}
		};

		Self { loc, dir, dist }
	}

	pub fn new_uncached(pos: EntityVec, dir: EntityVec) -> Self {
		Self::new_at(EntityLocation::new_uncached(pos), dir)
	}

	pub fn loc(&mut self) -> &mut EntityLocation {
		&mut self.loc
	}

	pub fn pos(&self) -> EntityVec {
		self.loc.pos()
	}

	pub fn dir(&self) -> EntityVec {
		self.dir
	}

	pub fn dist(&self) -> f64 {
		self.dist
	}

	pub fn step(
		&mut self,
		cx: (&VoxelWorldData, &CelledStorageView<VoxelChunkData>),
	) -> SmallVec<[RayCastIntersection; 3]> {
		let mut intersections = SmallVec::<[RayCastIntersection; 3]>::new();

		// Collect intersections
		let mut block_loc = self.loc.as_block_location();
		{
			let step_line = Line3::new_origin_delta(self.pos(), self.dir);
			self.loc.move_relative(cx, self.dir);

			let start_block = step_line.start.block_pos();
			let end_block = step_line.end.block_pos();
			let block_delta = end_block - start_block;

			for axis in Axis3::variants() {
				let delta = block_delta.comp(axis);
				debug_assert!((-1..=1).contains(&delta));

				let sign = match Sign::of(delta) {
					Some(sign) => sign,
					None => continue,
				};

				let face = BlockFace::compose(axis, sign);

				let isect_layer = start_block.block_interface_layer(face);
				let (isect_lerp, isect_pos) = axis.plane_intersect(isect_layer, step_line);

				intersections.push(RayCastIntersection {
					_non_exhaustive: (),
					block: block_loc, // This will be updated in a bit.
					face,
					distance: self.dist + isect_lerp,
					pos: isect_pos,
				});
			}

			intersections.sort_by(|a, b| a.distance.total_cmp(&b.distance));
		}

		// Update block positions
		for isect in &mut intersections {
			isect.block = block_loc.at_neighbor(cx, isect.face);
			block_loc = isect.block;
		}

		// Update distance accumulator
		// N.B. the direction is either normalized, in which case the step was of length 1, or we're
		// traveling with direction zero, in which case the distance is already infinite.
		self.dist += 1.;

		intersections
	}

	pub fn step_for<'a>(
		&'a mut self,
		cx: (&'a VoxelWorldData, &'a CelledStorageView<VoxelChunkData>),
		max_dist: f64,
	) -> RayCastIter<'a> {
		ContextualRayCastIter::new(max_dist).with_context((cx, self))
	}
}

#[derive(Debug, Clone)]
pub struct RayCastIntersection {
	_non_exhaustive: (),
	pub block: BlockLocation,
	pub face: BlockFace,
	pub pos: EntityVec,
	pub distance: f64,
}

pub type RayCastIter<'a> = WithContext<
	(
		(&'a VoxelWorldData, &'a CelledStorageView<VoxelChunkData>),
		&'a mut RayCast,
	),
	ContextualRayCastIter,
>;

#[derive(Debug, Clone)]
pub struct ContextualRayCastIter {
	pub max_dist: f64,
	back_log: SmallVec<[RayCastIntersection; 3]>,
}

impl ContextualRayCastIter {
	pub fn new(max_dist: f64) -> Self {
		Self {
			max_dist,
			back_log: Default::default(),
		}
	}
}

impl<'a>
	ContextualIter<(
		(&'a VoxelWorldData, &'a CelledStorageView<VoxelChunkData>),
		&'a mut RayCast,
	)> for ContextualRayCastIter
{
	type Item = RayCastIntersection;

	fn next_on_ref(
		&mut self,
		(cx, ray): &mut (
			(&'a VoxelWorldData, &'a CelledStorageView<VoxelChunkData>),
			&'a mut RayCast,
		),
	) -> Option<Self::Item> {
		let cx = *cx;

		while self.back_log.is_empty() {
			self.back_log = ray.step(cx);
		}

		let next = if !self.back_log.is_empty() {
			self.back_log.remove(0)
		} else if ray.dist() < self.max_dist {
			self.back_log = ray.step(cx);

			// It is possible that the ray needs to travel more than 1 unit to get out of a block.
			// The furthest it can travel is `sqrt(3 * 1^2) ~= 1.7` so we have to call this method
			// at most twice. Also, yes, this handles a zero-vector direction because rays with no
			// direction automatically get infinite distance.
			if self.back_log.is_empty() {
				self.back_log = ray.step(cx);
			}

			self.back_log.remove(0)
		} else {
			return None;
		};

		if next.distance <= self.max_dist {
			Some(next)
		} else {
			None
		}
	}
}
