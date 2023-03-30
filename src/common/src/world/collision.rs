use bort::{storage, Entity, Storage};
use crucible_util::{
	lang::{
		generator::{ContinuationSig, Yield},
		iter::ContextualIter,
		std_traits::VecLike,
	},
	mem::c_enum::CEnum,
	use_generator, yielder,
};
use smallvec::SmallVec;
use typed_glam::{glam::DVec2, traits::NumericVector};

use crate::{
	material::MaterialRegistry,
	math::{
		AaQuad, Aabb3, Axis3, BlockFace, EntityAabb, EntityVec, EntityVecExt, Line3, Sign,
		VecCompExt, WorldVecExt,
	},
	world::data::BlockState,
};

use super::{
	data::{BlockLocation, EntityLocation, VoxelWorldData},
	mesh::{QuadMeshLayer, VolumetricMeshLayer},
};

// === General Collisions === //

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

pub fn filter_all_colliders() -> impl FnMut(CollisionMeta) -> bool {
	|_| true
}

// === Block Collisions === //

#[derive(Debug, Clone)]
pub enum MaterialColliderDescriptor {
	Transparent,
	Cubic(CollisionMeta),
	Custom {
		volumes: VolumetricMeshLayer<CollisionMeta>,
		extra_quads: QuadMeshLayer<CollisionMeta>,
	},
}

impl MaterialColliderDescriptor {
	pub fn custom_from_volumes(volumes: VolumetricMeshLayer<CollisionMeta>) -> Self {
		Self::Custom {
			volumes,
			extra_quads: QuadMeshLayer::default(),
		}
	}
}

pub async fn occluding_volumes_in_block<'a>(
	y: &'a Yield<(EntityAabb, CollisionMeta)>,
	collider_descs: &'static Storage<MaterialColliderDescriptor>,
	world: &'a VoxelWorldData,
	registry: &'a MaterialRegistry,
	mut block: BlockLocation,
) {
	// Decode descriptor
	let Some(state) = block.state(world).filter(BlockState::is_not_air) else {
			return;
		};

	let descriptor = registry.resolve_slot(state.material);
	let descriptor = collider_descs.get(descriptor);

	// Determine collision volumes
	match &*descriptor {
		MaterialColliderDescriptor::Transparent => {}
		MaterialColliderDescriptor::Cubic(meta) => {
			y.produce((
				Aabb3 {
					origin: block.pos().negative_most_corner(),
					size: EntityVec::ONE,
				},
				*meta,
			))
			.await;
		}
		MaterialColliderDescriptor::Custom { volumes, .. } => {
			y.produce_many(volumes.iter_cloned().map(|(aabb, meta)| {
				(
					Aabb3 {
						origin: aabb.origin.cast::<EntityVec>()
							+ block.pos().negative_most_corner(),
						size: aabb.size.cast(),
					},
					meta,
				)
			}))
			.await;
		}
	}
}

pub async fn occluding_faces_in_block<'a>(
	y: &'a Yield<(AaQuad<EntityVec>, CollisionMeta)>,
	collider_descs: &'static Storage<MaterialColliderDescriptor>,
	world: &'a VoxelWorldData,
	registry: &'a MaterialRegistry,
	mut block: BlockLocation,
	face: BlockFace,
) {
	// Decode descriptor
	let Some(state) = block.state(world).filter(BlockState::is_not_air) else {
		return;
	};

	let descriptor = registry.resolve_slot(state.material);
	let descriptor = collider_descs.get(descriptor);

	// Determine an offset from block-relative coordinates to world coordinates
	let quad_offset = block.pos().negative_most_corner();

	// Determine collision volumes
	match &*descriptor {
		MaterialColliderDescriptor::Transparent => {}
		MaterialColliderDescriptor::Cubic(meta) => {
			y.produce((
				Aabb3 {
					origin: quad_offset,
					size: EntityVec::ONE,
				}
				.quad(face),
				*meta,
			))
			.await;
		}
		MaterialColliderDescriptor::Custom {
			volumes,
			extra_quads,
		} => {
			// Yield faces from volumes
			for (aabb, meta) in volumes.iter_cloned() {
				y.produce((
					Aabb3 {
						origin: aabb.origin.cast::<EntityVec>() + quad_offset,
						size: aabb.size.cast(),
					}
					.quad(face),
					meta,
				))
				.await;
			}

			// Yield additional faces
			for (quad, material) in extra_quads
				.iter_cloned()
				.filter(|(quad, _)| quad.face == face)
			{
				let AaQuad {
					origin,
					size: (sx, sy),
					..
				} = quad;

				y.produce((
					AaQuad {
						origin: origin.cast::<EntityVec>() + quad_offset,
						face,
						size: (sx.into(), sy.into()),
					},
					material,
				))
				.await;
			}
		}
	}
}

#[derive(Debug, Copy, Clone)]
pub struct IntersectingFaceInBlock {
	pub pos: EntityVec,
	pub face: BlockFace,
	pub dist_along_ray: f64,
	pub meta: CollisionMeta,
}

pub async fn intersecting_faces_in_block<'a>(
	y: &'a Yield<IntersectingFaceInBlock>,
	collider_descs: &'static Storage<MaterialColliderDescriptor>,
	world: &'a VoxelWorldData,
	registry: &'a MaterialRegistry,
	block: BlockLocation,
	line: Line3,
) {
	let delta = line.signed_delta();

	for axis in Axis3::variants() {
		let Some(sign) = Sign::of(delta.comp(axis)) else {
			continue;
		};
		let face = BlockFace::compose(axis, sign);
		use_generator!(let iter[y] = occluding_faces_in_block(
			y,
			collider_descs,
			world,
			registry,
			block,
			face
		));

		for (quad, meta) in iter.with_context(()) {
			let Some((lerp, pos)) = quad.intersection(line) else {
				continue
			};

			y.produce(IntersectingFaceInBlock {
				pos,
				face,
				// N.B. This lerp value is the actual length along the ray because the ray is a
				// unit vector.
				dist_along_ray: lerp,
				meta,
			})
			.await;
		}
	}
}

// === Volumetric Collisions === //

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

	// Find the maximum allowed delta
	use_generator!(let iter[y] = async {
		y.bind_empty_context();

		// For every block in the volume of potential occluders...
		for pos in check_aabb.iter_blocks() {
			use_generator!(let iter[y] = occluding_faces_in_block(
				y,
				collider_descs,
				world,
				registry,
				cached_loc.at_absolute(world, pos),
				quad.face.invert(),
			));

			// For every occluder produced by that block...
			for (occluder_quad, occluder_meta) in iter.with_context(()) {
				// Filter occluders by whether we are affected by them.
				if !filter(occluder_meta) {
					continue;
				}

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
				y.produce(rel_depth.max(0.0)).await;
			}
		}
	});

	iter.with_context(())
		.min_by(f64::total_cmp)
		.unwrap_or(delta)
		.min(delta)
}

pub async fn check_volume<'a>(
	y: &'a Yield<(BlockLocation, CollisionMeta)>,
	collider_descs: &'static Storage<MaterialColliderDescriptor>,
	world: &'a VoxelWorldData,
	registry: &'a MaterialRegistry,
	aabb: EntityAabb,
) {
	let cached_loc = BlockLocation::new(world, aabb.origin.block_pos());

	for block_pos in aabb.as_blocks().iter_blocks() {
		let block = cached_loc.at_absolute(world, block_pos);
		use_generator!(let iter[y] = occluding_volumes_in_block(
			y,
			collider_descs,
			world,
			registry,
			block,
		));

		for (block_aabb, meta) in iter.with_context(()) {
			if aabb.intersects(block_aabb) {
				y.produce((block, meta)).await;
			}
		}
	}
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
				(dir * dir_len_recip, 0.)
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

	pub fn step_line(&self) -> Line3 {
		Line3::new_origin_delta(self.pos(), self.dir)
	}

	pub fn step(&mut self, world: &VoxelWorldData) -> SmallVec<[RayCastIntersection; 3]> {
		// Construct a buffer to hold our `RayCastIntersection`s. There should be at most three of
		// these and, therefore, we should never allocate anything on the heap.
		let mut intersections = SmallVec::<[RayCastIntersection; 3]>::new();

		// We begin by storing a pointer to our starting block position. This will be useful later
		// when we try to determine the position of the blocks being intersected.
		let mut block_loc = self.loc.as_block_location();

		// Collect intersections
		{
			// Construct a line from our active `loc` to its end after we performed a step.
			let step_line = self.step_line();

			// Bump our location by this delta.
			self.loc.move_relative(world, self.dir);

			// Determine the delta in blocks that we committed by taking this step. This is not
			// necessarily from one neighbor to the other in the case where we cast along the
			// diagonal of a block. However, we do know that we will step along each axis at most
			// once. This is why it's important that our step delta remains less than or equal to `1`:
			// in the worst case scenario where our step delta is axis-aligned, a delta greater than
			// `1` could skip entirely past a block!
			let start_block = step_line.start.block_pos();
			let end_block = step_line.end.block_pos();
			let block_delta = end_block - start_block;

			// For every axis...
			for axis in Axis3::variants() {
				// Determine the face through which we traveled.
				let delta = block_delta.comp(axis);
				debug_assert!((-1..=1).contains(&delta));

				let Some(sign) = Sign::of(delta) else {
					continue;
				};

				let face = BlockFace::compose(axis, sign);

				// Determine the plane of intersection.
				// N.B., because this step can only go through each axis at most once, taking the
				// plane w.r.t the `start_block` is identical to taking the plane w.r.t the actual
				// block from which the ray is traveling.
				let isect_plane = start_block.face_plane(face);

				// Now, we just have to determine the point of intersection and commit it to the
				// buffer.
				let (isect_lerp, isect_pos) = isect_plane.intersection(step_line);

				intersections.push(RayCastIntersection {
					block: block_loc, // This will be updated in a bit.
					face,
					// N.B. This lerp value is the actual length along the ray because the ray is a
					// unit vector.
					distance: self.dist + isect_lerp,
					pos: isect_pos,
				});
			}
		}

		// We then sort our intersection list by distance traveled to both satisfy the contract of
		// this method and to ensure that we can accurately determine block positions.
		intersections.sort_by(|a, b| a.distance.total_cmp(&b.distance));

		// Update block positions by accumulating face traversals onto `block_loc`—which is currently
		// just the position of our ray before the step began.
		for isect in &mut intersections {
			isect.block = block_loc.at_neighbor(world, isect.face);
			block_loc = isect.block;
		}

		// Finally, we update distance accumulator.
		// N.B. the direction is either normalized, in which case the step was of length 1, or we're
		// traveling with direction zero, in which case the distance is already infinite.
		self.dist += 1.0;

		intersections
	}

	pub fn step_intersect(
		&mut self,
		collider_descs: &'static Storage<MaterialColliderDescriptor>,
		world: &VoxelWorldData,
		registry: &MaterialRegistry,
		isect_buffer: &mut impl VecLike<Elem = (RayCastIntersection, CollisionMeta)>,
	) {
		let start_dist = self.dist();
		let line = self.step_line();
		let block_isects = self.step(world);

		// Clear scratch buffer
		isect_buffer.clear();

		// Collect intersections
		for block_isect in block_isects {
			use_generator!(let iter[y] = intersecting_faces_in_block(
				y,
				collider_descs,
				world,
				registry,
				block_isect.block,
				line,
			));

			for face_isect in iter
				.with_context(())
				.filter(|isect| (0.0..=1.0).contains(&isect.dist_along_ray))
			{
				isect_buffer.push((
					RayCastIntersection {
						block: block_isect.block,
						face: face_isect.face,
						pos: face_isect.pos,
						distance: start_dist + face_isect.dist_along_ray,
					},
					face_isect.meta,
				));
			}
		}

		// Sort intersections
		isect_buffer.sort_by(|a, b| a.0.distance.total_cmp(&b.0.distance));
	}

	pub async fn step_for(
		&mut self,
		y: &yielder![RayCastIntersection; for<'a> &'a VoxelWorldData],
		max_dist: f64,
	) {
		while self.dist() <= max_dist {
			for isect in y.ask(|world| self.step(world)).await {
				if isect.distance > max_dist {
					continue;
				}

				y.produce(isect).await;
			}
		}
	}

	pub async fn step_intersect_for(
		&mut self,
		y: &yielder![(RayCastIntersection, CollisionMeta); for<'a> (&'a VoxelWorldData, &'a MaterialRegistry)],
		collider_descs: &'static Storage<MaterialColliderDescriptor>,
		max_dist: f64,
	) {
		let mut isect_buffer = SmallVec::<[_; 4]>::new();

		while self.dist() <= max_dist {
			y.ask(|(world, registry)| {
				self.step_intersect(collider_descs, world, registry, &mut isect_buffer)
			})
			.await;

			for (isect, meta) in &isect_buffer {
				if isect.distance > max_dist {
					return;
				}

				y.produce((isect.clone(), *meta)).await;
			}
		}
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

// === Rigid Body === //

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
