use std::{borrow::BorrowMut, iter};

use bort::{storage, CompRef, Entity, Storage};
use crucible_util::{choice_iter, lang::iter::ContextualIter, match_unwrap, mem::c_enum::CEnum};
use smallvec::SmallVec;
use typed_glam::glam::DVec2;

use crate::{
	material::{MaterialRegistry, AIR_MATERIAL_SLOT},
	world::math::{Aabb3, Axis3, BlockFace, EntityVecExt, Line3, Sign, Vec3Ext, WorldVecExt},
};

use super::{
	data::{BlockLocation, EntityLocation, VoxelWorldData},
	math::{AaQuad, EntityAabb, EntityVec},
	mesh::{QuadMeshLayer, StyledQuad},
};

// === Colliders === //

#[derive(Debug, Copy, Clone)]
pub struct CollisionMeta {
	pub mask: u64,
	pub meta: Option<Entity>,
}

impl CollisionMeta {
	pub const OPAQUE: Self = Self {
		mask: u64::MAX,
		meta: None,
	};
}

#[derive(Debug, Clone)]
pub enum MaterialColliderDescriptor {
	Transparent,
	Cubic(CollisionMeta),
	Mesh(QuadMeshLayer<CollisionMeta>),
}

pub fn colliders_in_block(
	collider_descs: &'static Storage<MaterialColliderDescriptor>,
	world: &VoxelWorldData,
	registry: &MaterialRegistry,
	block: &mut BlockLocation,
	face: BlockFace,
) -> impl Iterator<Item = (AaQuad<EntityVec>, CollisionMeta)> {
	choice_iter!(Iter: Empty, Fixed, Registry);

	// Decode the block state
	let Some(state) = block.state(world).filter(|state| state.material != AIR_MATERIAL_SLOT) else {
		return Iter::Empty(iter::empty());
	};

	let descriptor = registry.resolve_slot(state.material);
	let descriptor = collider_descs.get(descriptor);

	// Determine an offset from block-relative coordinates to world coordinates
	let quad_offset = block.pos().negative_most_corner();

	// Iterate quads from the block collision descriptor
	match &*descriptor {
		MaterialColliderDescriptor::Transparent => Iter::Empty(iter::empty()),
		MaterialColliderDescriptor::Cubic(meta) => Iter::Fixed(iter::once((
			Aabb3 {
				origin: quad_offset,
				size: EntityVec::ONE,
			}
			.quad(face),
			*meta,
		))),
		MaterialColliderDescriptor::Mesh(_) => {
			let mesh = CompRef::map(descriptor, |descriptor| {
				match_unwrap!(MaterialColliderDescriptor::Mesh(mesh) = descriptor);
				mesh
			});
			let mut i = 0;
			let iter = iter::from_fn(move || {
				let StyledQuad {
					quad: AaQuad {
						origin,
						face,
						size: (sx, sy),
					},
					material: face_meta,
				} = *mesh.quads.get(i)?;

				i += 1;

				Some((
					AaQuad {
						origin: quad_offset + origin.as_dvec3(),
						face,
						size: (sx.into(), sy.into()),
					},
					face_meta,
				))
			});

			Iter::Registry(iter.filter(move |(quad, _)| quad.face == face))
		}
	}
}

pub fn filter_all_colliders() -> impl FnMut(CollisionMeta) -> bool {
	|_| true
}

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

	pub fn step(&mut self, world: &VoxelWorldData) -> SmallVec<[RayCastIntersection; 3]> {
		let mut intersections = SmallVec::<[RayCastIntersection; 3]>::new();

		// Collect intersections
		let mut block_loc = self.loc.as_block_location();
		{
			let step_line = Line3::new_origin_delta(self.pos(), self.dir);
			self.loc.move_relative(world, self.dir);

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
			isect.block = block_loc.at_neighbor(world, isect.face);
			block_loc = isect.block;
		}

		// Update distance accumulator
		// N.B. the direction is either normalized, in which case the step was of length 1, or we're
		// traveling with direction zero, in which case the distance is already infinite.
		self.dist += 1.;

		intersections
	}

	pub fn into_iter(self, dist: f64) -> RayCastIter<Self> {
		RayCastIter::new(self, dist)
	}

	pub fn step_for(&mut self, dist: f64) -> RayCastIter<&'_ mut Self> {
		RayCastIter::new(self, dist)
	}
}

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct RayCastIntersection {
	pub block: BlockLocation,
	pub face: BlockFace,
	pub pos: EntityVec,
	pub distance: f64,
}

#[derive(Debug)]
pub struct RayCastIter<R> {
	pub ray: R,
	pub max_dist: f64,
	back_log: SmallVec<[RayCastIntersection; 3]>,
}

impl<R> RayCastIter<R> {
	pub fn new(ray: R, max_dist: f64) -> Self {
		Self {
			ray,
			max_dist,
			back_log: Default::default(),
		}
	}
}

impl<'a, R: BorrowMut<RayCast>> ContextualIter<&'a VoxelWorldData> for RayCastIter<R> {
	type Item = RayCastIntersection;

	fn next_on_ref(&mut self, world: &mut &'a VoxelWorldData) -> Option<Self::Item> {
		let world = *world;

		let next = if !self.back_log.is_empty() {
			self.back_log.remove(0)
		} else if self.ray.borrow().dist() < self.max_dist {
			self.back_log = self.ray.borrow_mut().step(world);

			// It is possible that the ray needs to travel more than 1 unit to get out of a block.
			// The furthest it can travel is `sqrt(3 * 1^2) ~= 1.7` so we have to call this method
			// at most twice. Also, yes, this handles a zero-vector direction because rays with no
			// direction automatically get infinite distance.
			if self.back_log.is_empty() {
				self.back_log = self.ray.borrow_mut().step(world);
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

// === Collisions === //

pub const COLLISION_TOLERANCE: f64 = 0.0005;

pub fn cast_volume(
	world: &VoxelWorldData,
	registry: &MaterialRegistry,
	quad: AaQuad<EntityVec>,
	delta: f64,
	tolerance: f64,
	mut filter: impl FnMut(CollisionMeta) -> bool,
) -> f64 {
	let collider_descs = storage();

	// N.B. to ensure that `tolerance` is respected, we have to increase our check volume by
	// `tolerance` so that we catch blocks that are outside the check volume but may nonetheless
	// wish to enforce a tolerance margin of their own.
	//
	// We do this to prevent the following scenario:
	//
	// ```
	// 0   1   2
	// *---*---*
	// | %--->*|
	// *---*---*
	//       |--   Let's say this is the required tolerance...
	//   |----|    ...and this is the actual movement delta.
	//
	// Because our movement delta never hits the block face at `x = 2`, it never requires the face to
	// contribute to tolerance checking, allowing us to bypass its tolerance and eventually "tunnel"
	// through the occluder.
	// ```
	//
	// If these additional blocks don't contribute to collision detection with their tolerance, we'll
	// just ignore them.
	let check_aabb = quad.extrude_hv(delta + tolerance).as_blocks();
	let cached_loc = BlockLocation::new(world, check_aabb.origin);

	// Return the allowed delta.
	let mut max_delta = delta;

	for pos in check_aabb.iter_blocks() {
		for (occluder_quad, occluder_meta) in colliders_in_block(
			collider_descs,
			world,
			registry,
			&mut cached_loc.at_absolute(world, pos),
			quad.face.invert(),
		) {
			// Filter occluders by whether we are affected by them.
			if !filter(occluder_meta) {
				continue;
			}

			// Determine the maximum distance allowed by this occluder
			let new_max_delta = {
				if !occluder_quad
					.as_rect::<DVec2>()
					.intersects(quad.as_rect::<DVec2>())
				{
					continue;
				}

				// Find its depth along the axis of movement
				let my_depth = occluder_quad.origin.comp(quad.face.axis());

				// And compare that to the starting depth to find the maximum allowable distance.
				// FIXME: We may be comparing against faces behind us!
				let rel_depth = (my_depth - quad.origin.comp(quad.face.axis())).abs();

				// Now, provide `tolerance`.
				// This step also ensures that, if the block plus its tolerance is outside of our delta,
				// it will have essentially zero effect on collision detection.
				let rel_depth = rel_depth - tolerance;

				// If `rel_depth` is negative, we were not within tolerance to begin with. We trace
				// this scenario for debug purposes.

				// It has to be a somewhat big bypass to be reported. Floating point errors are
				// expected—after all, that's why we have a tolerance in the first place.
				if rel_depth < -tolerance / 2.0 {
					log::trace!(
						"cast_volume(quad: {quad:?}, delta: {delta}, tolerance: {tolerance}) could \
						not keep collider within tolerance. Bypass depth: {}",
						-rel_depth,
					);
				}

				// We still clamp this to `[0..)`.
				rel_depth.max(0.0)
			};

			max_delta = max_delta.min(new_max_delta);
		}
	}

	max_delta
}

pub fn move_rigid_body(
	world: &VoxelWorldData,
	registry: &MaterialRegistry,
	mut aabb: EntityAabb,
	delta: EntityVec,
	mut filter: impl FnMut(CollisionMeta) -> bool,
) -> EntityVec {
	for axis in Axis3::variants() {
		// Decompose the movement part
		let signed_delta = delta.comp(axis);
		let unsigned_delta = signed_delta.abs();
		let sign = Sign::of(signed_delta).unwrap_or(Sign::Positive);
		let face = BlockFace::compose(axis, sign);

		// Determine how far we can move
		let actual_delta = cast_volume(
			world,
			registry,
			aabb.quad(face),
			unsigned_delta,
			COLLISION_TOLERANCE,
			&mut filter,
		);

		// Commit the movement
		aabb.origin += face.unit_typed::<EntityVec>() * actual_delta;
	}

	aabb.origin
}
