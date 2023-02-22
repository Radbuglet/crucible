use bort::{Entity, OwnedEntity};
use crucible_util::{
	lang::{
		iter::{ContextualIter, WithContext},
		polyfill::OptionPoly,
	},
	mem::c_enum::CEnum,
};
use smallvec::SmallVec;
use typed_glam::traits::{CastVecFrom, SignedNumericVector3};

use crate::voxel::math::{Axis3, BlockFace, EntityVecExt, Line3, Sign, Vec3Ext, WorldVecExt};

use super::{
	data::{BlockState, VoxelWorldData, AIR_MATERIAL_SLOT},
	math::{AaQuad, Aabb, ChunkVec, EntityAabb, EntityVec, WorldVec},
};

// === Location === //

pub type BlockLocation = Location<WorldVec>;
pub type EntityLocation = Location<EntityVec>;

#[derive(Debug, Copy, Clone)]
pub struct Location<V> {
	pos: V,
	chunk_cache: Option<Entity>,
}

impl<V> Location<V>
where
	WorldVec: CastVecFrom<V>,
	V: CastVecFrom<WorldVec>,
	V: SignedNumericVector3,
{
	pub fn new(world: &VoxelWorldData, pos: V) -> Self {
		Self {
			pos,
			chunk_cache: world.get_chunk(WorldVec::cast_from(pos).chunk()),
		}
	}

	pub fn new_uncached(pos: V) -> Self {
		Self {
			pos,
			chunk_cache: None,
		}
	}

	pub fn refresh(&mut self, world: &VoxelWorldData) {
		self.chunk_cache = world.get_chunk(WorldVec::cast_from(self.pos).chunk());
	}

	pub fn pos(&self) -> V {
		self.pos
	}

	pub fn set_pos_within_chunk(&mut self, pos: V) {
		debug_assert_eq!(
			WorldVec::cast_from(pos).chunk(),
			WorldVec::cast_from(self.pos).chunk()
		);

		self.pos = pos;
	}

	pub fn chunk(&mut self, world: &VoxelWorldData) -> Option<Entity> {
		match self.chunk_cache {
			Some(chunk) => Some(chunk),
			None => {
				self.refresh(world);
				self.chunk_cache
			}
		}
	}

	pub fn move_to_neighbor(&mut self, world: &VoxelWorldData, face: BlockFace) {
		// Update position
		let old_pos = self.pos;
		self.pos += face.unit_typed::<V>();

		// Update chunk cache
		if WorldVec::cast_from(old_pos).chunk() != WorldVec::cast_from(self.pos).chunk() {
			if let Some(chunk) = self.chunk_cache {
				self.chunk_cache = world.chunk_state(chunk).neighbor(face);
			} else {
				self.refresh(world);
			}
		}
	}

	pub fn at_neighbor(mut self, world: &VoxelWorldData, face: BlockFace) -> Self {
		self.move_to_neighbor(world, face);
		self
	}

	pub fn move_to(&mut self, world: &VoxelWorldData, new_pos: V) {
		let chunk_delta =
			WorldVec::cast_from(new_pos).chunk() - WorldVec::cast_from(self.pos).chunk();

		if let (Some(chunk), Some(face)) =
			(self.chunk_cache, BlockFace::from_vec(chunk_delta.to_glam()))
		{
			self.pos = new_pos;
			self.chunk_cache = world.chunk_state(chunk).neighbor(face);
		} else {
			self.pos = new_pos;
			self.refresh(world);
		}
	}

	pub fn at_absolute(mut self, world: &VoxelWorldData, new_pos: V) -> Self {
		self.move_to(world, new_pos);
		self
	}

	pub fn move_relative(&mut self, world: &VoxelWorldData, delta: V) {
		self.move_to(world, self.pos + delta);
	}

	pub fn at_relative(mut self, world: &VoxelWorldData, delta: V) -> Self {
		self.move_relative(world, delta);
		self
	}

	pub fn state(&mut self, world: &VoxelWorldData) -> Option<BlockState> {
		self.chunk(world).map(|chunk| {
			world
				.chunk_state(chunk)
				.block_state(WorldVec::cast_from(self.pos).block())
		})
	}

	pub fn set_state(&mut self, world: &mut VoxelWorldData, state: BlockState) {
		let chunk = match self.chunk(world) {
			Some(chunk) => chunk,
			None => {
				log::warn!("`set_state` called on `BlockLocation` outside of the world.");
				return;
			}
		};

		world
			.chunk_state_mut(chunk)
			.set_block_state(WorldVec::cast_from(self.pos).block(), state);
	}

	pub fn set_state_or_create(
		&mut self,
		world: &mut VoxelWorldData,
		factory: impl FnOnce(ChunkVec) -> OwnedEntity,
		state: BlockState,
	) {
		// Fetch chunk
		let chunk = match self.chunk(world) {
			Some(chunk) => chunk,
			None => {
				let pos = WorldVec::cast_from(self.pos).chunk();
				let (chunk, chunk_ref) = factory(pos).split_guard();
				world.add_chunk(pos, chunk);
				chunk_ref
			}
		};

		// Set block state
		world
			.chunk_state_mut(chunk)
			.set_block_state(WorldVec::cast_from(self.pos).block(), state);
	}

	pub fn as_block_location(&self) -> BlockLocation {
		BlockLocation {
			chunk_cache: self.chunk_cache,
			pos: WorldVec::cast_from(self.pos),
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

	pub fn step_for<'a>(&'a mut self, world: &'a VoxelWorldData, max_dist: f64) -> RayCastIter<'a> {
		ContextualRayCastIter::new(max_dist).with_context((world, self))
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

pub type RayCastIter<'a> =
	WithContext<(&'a VoxelWorldData, &'a mut RayCast), ContextualRayCastIter>;

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

impl<'a> ContextualIter<(&'a VoxelWorldData, &'a mut RayCast)> for ContextualRayCastIter {
	type Item = RayCastIntersection;

	fn next_on_ref(
		&mut self,
		(world, ray): &mut (&'a VoxelWorldData, &'a mut RayCast),
	) -> Option<Self::Item> {
		let world = *world;

		let next = if !self.back_log.is_empty() {
			self.back_log.remove(0)
		} else if ray.dist() < self.max_dist {
			self.back_log = ray.step(world);

			// It is possible that the ray needs to travel more than 1 unit to get out of a block.
			// The furthest it can travel is `sqrt(3 * 1^2) ~= 1.7` so we have to call this method
			// at most twice. Also, yes, this handles a zero-vector direction because rays with no
			// direction automatically get infinite distance.
			if self.back_log.is_empty() {
				self.back_log = ray.step(world);
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

pub fn cast_volume(world: &VoxelWorldData, quad: AaQuad<EntityVec>, delta: f64) -> f64 {
	let check_aabb = quad.extrude_hv(delta).as_blocks();

	let loc = BlockLocation::new(world, check_aabb.origin);

	check_aabb
		.iter_blocks()
		// See how far we can travel
		.map(|pos| {
			if loc
				.at_absolute(world, pos)
				.state(world)
				.p_is_none_or(|v| v.material == AIR_MATERIAL_SLOT)
			{
				delta
			} else {
				0.0
			}
		})
		// And take the minimum distance
		.min_by(f64::total_cmp)
		// This is safe to unwrap because all entity AABBs will cover at least one block.
		.unwrap()
}

pub fn move_rigid_body(
	world: &VoxelWorldData,
	mut aabb: EntityAabb,
	delta: EntityVec,
) -> EntityVec {
	for axis in Axis3::variants() {
		// Decompose the movement part
		let signed_delta = delta.comp(axis);
		let unsigned_delta = signed_delta.abs();
		let sign = Sign::of(signed_delta).unwrap_or(Sign::Positive);
		let face = BlockFace::compose(axis, sign);

		// Determine how far we can move
		let actual_delta = cast_volume(world, aabb.quad(face), unsigned_delta);

		// Commit the movement
		aabb.origin += face.unit_typed::<EntityVec>() * actual_delta;
	}

	aabb.origin
}

pub fn move_rigid_body_relative(
	world: &VoxelWorldData,
	origin: EntityVec,
	origin_offset: EntityVec,
	size: EntityVec,
	delta: EntityVec,
) -> EntityVec {
	move_rigid_body(
		world,
		Aabb {
			origin: origin - origin_offset,
			size,
		},
		delta,
	) + origin_offset
}
