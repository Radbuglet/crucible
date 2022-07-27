use std::{
	cell::{Cell, RefCell},
	collections::{HashMap, HashSet},
	hash,
};

use crucible_core::{
	array::arr,
	c_enum::ExposesVariants,
	contextual_iter::{ContextualIter, WithContext},
};
use geode::prelude::*;
use smallvec::SmallVec;

use super::math::{
	Axis3, BlockFace, BlockVec, BlockVecExt, ChunkVec, EntityVec, EntityVecExt, Line3, Sign,
	WorldVec, WorldVecExt, CHUNK_VOLUME,
};

// === Voxel Data Containers === //

type WorldEntity = EntityWith<VoxelWorldData>;
type ChunkEntity = EntityWith<VoxelChunkData>;

pub type ChunkFactory = dyn Factory<ChunkFactoryRequest, Owned<ChunkEntity>>;

#[derive(Debug)]
pub struct ChunkFactoryRequest {
	pub world: WorldEntity,
}

pub struct VoxelWorldData {
	me: WorldEntity,
	chunk_factory: MaybeOwned<Obj<ChunkFactory>>,
	inner: RefCell<VoxelWorldDataInner>,
}

// TODO: Move to `Copy` containers.
#[derive(Default)]
struct VoxelWorldDataInner {
	chunks: HashMap<ChunkVec, Owned<ChunkEntity>>,
	dirty_chunks: HashSet<ChunkEntity>,
}

impl VoxelWorldData {
	pub fn new(me: Entity, chunk_factory: MaybeOwned<Obj<ChunkFactory>>) -> Self {
		Self {
			me: WorldEntity::force_cast(me),
			chunk_factory,
			inner: Default::default(),
		}
	}

	pub fn me(&self) -> WorldEntity {
		self.me
	}

	pub fn add_chunk(
		&self,
		s: Session,
		pos: ChunkVec,
	) -> (ChunkEntity, Option<Owned<ChunkEntity>>) {
		let mut inner = self.inner.borrow_mut();

		// Create chunk
		let (chunk_guard, chunk) = self
			.chunk_factory
			.get(s)
			.create(s, ChunkFactoryRequest { world: self.me })
			.to_guard_ref_pair();

		let chunk_data = chunk_guard.comp(s);
		assert_eq!(chunk_data.world(), None);

		// Replace the old chunk with new chunk
		let replaced = inner.chunks.insert(pos, chunk_guard);
		if let Some(replaced) = replaced.as_ref() {
			replaced.comp(s).world.set(None);
		}

		// Update `chunk_data` state
		chunk_data.world.set(Some(self.me));
		chunk_data.position.set(pos);

		// Link new chunk to neighbors
		for face in BlockFace::variants() {
			let rel = ChunkVec::from_raw(face.unit());
			let neighbor_pos = pos + rel;
			let neighbor = inner
				.chunks
				.get(&neighbor_pos)
				.map(|neighbor| neighbor.weak_copy());

			// Link ourselves to the neighboring chunk
			chunk_data.neighbors[face.index()].set(neighbor);

			// Link the neighboring chunk to us
			if let Some(neighbor) = neighbor {
				neighbor.comp(s).neighbors[face.invert().index()].set(Some(chunk));
			}
		}

		(chunk, replaced)
	}

	pub fn get_chunk(&self, pos: ChunkVec) -> Option<ChunkEntity> {
		self.inner
			.borrow()
			.chunks
			.get(&pos)
			.map(|chunk| chunk.weak_copy())
	}

	pub fn get_chunk_or_add(&self, s: Session, pos: ChunkVec) -> ChunkEntity {
		if let Some(chunk) = self.get_chunk(pos) {
			chunk
		} else {
			self.add_chunk(s, pos).0
		}
	}

	pub fn flush_dirty_chunks(&self) -> HashSet<ChunkEntity> {
		std::mem::replace(&mut self.inner.borrow_mut().dirty_chunks, HashSet::new())
	}
}

#[derive(Debug)]
pub struct VoxelChunkData {
	me: ChunkEntity,
	world: Cell<Option<WorldEntity>>,
	neighbors: [Cell<Option<ChunkEntity>>; BlockFace::COUNT],
	position: Cell<ChunkVec>,
	blocks: Box<[Cell<u32>; CHUNK_VOLUME as usize]>,
}

impl VoxelChunkData {
	pub fn new(me: Entity) -> Self {
		Self {
			me: ChunkEntity::force_cast(me),
			world: Default::default(),
			neighbors: Default::default(),
			position: Default::default(),
			blocks: Box::new(arr![Cell::new(0); CHUNK_VOLUME as usize]),
		}
	}

	pub fn me(&self) -> ChunkEntity {
		self.me
	}

	pub fn world(&self) -> Option<WorldEntity> {
		self.world.get()
	}

	pub fn neighbor(&self, face: BlockFace) -> Option<ChunkEntity> {
		self.neighbors[face as usize].get()
	}

	pub fn pos(&self) -> ChunkVec {
		self.position.get()
	}

	pub fn get_block_state(&self, pos: BlockVec) -> BlockState {
		BlockState::decode(self.blocks[pos.to_index()].get())
	}

	pub fn set_block_state(&self, s: Session, pos: BlockVec, state: BlockState) {
		if let Some(world) = self.world() {
			world
				.comp(s)
				.inner
				.borrow_mut()
				.dirty_chunks
				.insert(self.me);
		}

		self.blocks[pos.to_index()].set(state.encode());
	}
}

// === Block State Manipulation === //

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Default)]
pub struct BlockState {
	pub material: u16,
	pub variant: u8,
	pub light_level: u8,
}

// Format:
//
// ```text
// LSB                                      MSB
// ---- ---- ~~~~ ~~~~ | ---- ---- | ~~~~ ~~~~ |
// Material Data       | Variant   | Light lvl |
// (u16)               | (u8)      | (u8)      |
// ```
impl BlockState {
	pub fn decode(word: u32) -> Self {
		let material = word as u16;
		let variant = word.to_be_bytes()[2];
		let light_level = word.to_be_bytes()[3];

		Self {
			material,
			variant,
			light_level,
		}
	}

	pub fn encode(&self) -> u32 {
		let mut enc = self.material as u32;
		enc += (self.variant as u32) << 16;
		enc += (self.material as u32) << 24;
		enc
	}
}

// === `BlockLocation` === //

#[derive(Debug, Copy, Clone)]
pub struct BlockLocation {
	chunk_cache: Option<ChunkEntity>,
	vec: WorldVec,
}

impl hash::Hash for BlockLocation {
	fn hash<H: hash::Hasher>(&self, state: &mut H) {
		self.vec.hash(state);
	}
}

impl Eq for BlockLocation {}

impl PartialEq for BlockLocation {
	fn eq(&self, other: &Self) -> bool {
		self.vec == other.vec
	}
}

impl BlockLocation {
	pub fn new_uncached(pos: WorldVec) -> Self {
		Self {
			chunk_cache: None,
			vec: pos,
		}
	}

	pub fn new_cached(world: &VoxelWorldData, pos: WorldVec) -> Self {
		let chunk_pos = pos.chunk();
		let chunk_cache = world.get_chunk(chunk_pos);
		Self {
			chunk_cache,
			vec: pos,
		}
	}

	pub fn vec(self) -> WorldVec {
		self.vec
	}

	pub fn update<F: FnOnce(WorldVec) -> WorldVec>(self, s: Session, f: F) -> Self {
		Self::move_to(self, s, f(self.vec()))
	}

	pub fn move_to(self, s: Session, pos: WorldVec) -> Self {
		self.move_by(s, pos - self.vec)
	}

	pub fn move_to_emit_delta(self, s: Session, pos: WorldVec) -> (Self, WorldVec) {
		let delta = pos - self.vec;
		let loc = self.move_by(s, delta);
		(loc, delta)
	}

	pub fn move_by(mut self, s: Session, delta: WorldVec) -> Self {
		for axis in Axis3::variants() {
			if let Some(sign) = Sign::of(delta[axis]) {
				self =
					self.neighbor_with_stride(s, BlockFace::compose(axis, sign), delta[axis].abs());
			}
		}
		self
	}

	pub fn neighbor(self, s: Session, face: BlockFace) -> Self {
		self.neighbor_with_stride(s, face, 1)
	}

	pub fn neighbor_with_stride(mut self, s: Session, face: BlockFace, stride: i32) -> Self {
		debug_assert!(stride >= 0);

		// Update position, keeping track of our chunk positions.
		let old_chunk_pos = self.vec.chunk();
		self.vec += face.unit() * stride;
		let new_chunk_pos = self.vec.chunk();

		// Attempt to update the chunk cache.
		let chunks_moved = (new_chunk_pos[face.axis()] - old_chunk_pos[face.axis()]).abs();

		if chunks_moved < 4 {
			// While we're still holding on to a cached chunk handle, navigate through its neighbors.
			for _ in 0..chunks_moved {
				let chunk_cache = match self.chunk_cache {
					Some(chunk_cache) => chunk_cache,
					None => break,
				};

				self.chunk_cache = chunk_cache.comp(s).neighbor(face);
			}
		} else {
			// We've moved too far. Invalidate the chunk cache.
			self.chunk_cache = None;
		}

		self
	}

	// === Chunk Querying === //

	pub fn invalidate_stale_cache(&mut self, s: Session, world_data: &VoxelWorldData) {
		// Ensure that our cached chunk is actually in the world.
		if let Some(chunk_cache) = self.chunk_cache {
			if chunk_cache.comp(s).world() != Some(world_data.me()) {
				self.chunk_cache = None;
			}
		}
	}

	pub fn recompute_cache(&mut self, s: Session, world_data: &VoxelWorldData) {
		self.invalidate_stale_cache(s, world_data);

		if self.chunk_cache.is_none() {
			self.chunk_cache = world_data.get_chunk(self.vec.chunk());
		}
	}

	pub fn recompute_cache_or_add(&mut self, s: Session, world_data: &VoxelWorldData) {
		self.invalidate_stale_cache(s, world_data);

		if self.chunk_cache.is_none() {
			self.chunk_cache = Some(world_data.get_chunk_or_add(s, self.vec.chunk()));
		}
	}

	pub fn chunk(&mut self, s: Session, world_data: &VoxelWorldData) -> Option<ChunkEntity> {
		self.recompute_cache(s, world_data);
		self.chunk_cache
	}

	pub fn chunk_or_add(&mut self, s: Session, world_data: &VoxelWorldData) -> ChunkEntity {
		self.recompute_cache_or_add(s, world_data);
		self.chunk_cache.unwrap()
	}

	// === Chunk Modification === //

	pub fn get_block_state(
		&mut self,
		s: Session,
		world_data: &VoxelWorldData,
	) -> Option<BlockState> {
		let chunk = self.chunk(s, world_data)?.comp(s);

		Some(chunk.get_block_state(self.vec.block()))
	}

	pub fn set_block_state(&mut self, s: Session, world_data: &VoxelWorldData, state: BlockState) {
		let chunk = self.chunk_or_add(s, world_data).comp(s);
		chunk.set_block_state(s, self.vec.block(), state);
	}
}

// === Voxel Ray Cast === //

#[derive(Debug, Clone)]
pub struct RayCast {
	b_loc: BlockLocation,
	f_pos: EntityVec,
	f_dir: EntityVec,
	dist: f64,
}

impl RayCast {
	pub fn new_with_ptr(origin_loc: BlockLocation, origin: EntityVec, dir: EntityVec) -> Self {
		debug_assert_eq!(origin_loc.vec(), origin.block_pos());

		let (dir, dist) = {
			let dir_len_recip = dir.length_recip();

			if dir_len_recip.is_finite() && dir_len_recip > 0.0 {
				(dir * dir_len_recip, 1.)
			} else {
				(EntityVec::ZERO, f64::INFINITY)
			}
		};

		Self {
			b_loc: origin_loc,
			f_pos: origin,
			f_dir: dir.normalize_or_zero(),
			dist,
		}
	}

	pub fn new_uncached(origin: EntityVec, dir: EntityVec) -> Self {
		Self::new_with_ptr(BlockLocation::new_uncached(origin.block_pos()), origin, dir)
	}

	pub fn new_cached(world_data: &VoxelWorldData, origin: EntityVec, dir: EntityVec) -> Self {
		Self::new_with_ptr(
			BlockLocation::new_cached(world_data, origin.block_pos()),
			origin,
			dir,
		)
	}

	pub fn block_loc(&mut self) -> &mut BlockLocation {
		&mut self.b_loc
	}

	pub fn f_pos(&self) -> EntityVec {
		self.f_pos
	}

	pub fn f_dir(&self) -> EntityVec {
		self.f_dir
	}

	pub fn dist(&self) -> f64 {
		self.dist
	}

	pub fn step(&mut self, s: Session) -> SmallVec<[RayCastIntersection; 3]> {
		debug_assert_eq!(self.b_loc.vec(), self.f_pos.block_pos());

		let mut intersections = SmallVec::<[RayCastIntersection; 3]>::new();

		// Collect intersections
		{
			let step_line = Line3::new_origin_delta(self.f_pos, self.f_dir);
			self.f_pos += self.f_dir;

			let start_block = step_line.start.block_pos();
			let end_block = step_line.end.block_pos();
			let block_delta = end_block - start_block;

			for axis in Axis3::variants() {
				let delta = block_delta[axis];
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
					block_loc: self.b_loc, // This will be updated in a bit.
					face,
					distance: self.dist + isect_lerp,
					pos: isect_pos,
				});
			}

			intersections.sort_by(|a, b| a.distance.total_cmp(&b.distance));
		}

		// Update block positions
		for isect in &mut intersections {
			isect.block_loc = self.b_loc.neighbor(s, isect.face);
			self.b_loc = isect.block_loc;
		}

		// Update distance accumulator
		// N.B. the direction is either normalized, in which case the step was of length 1, or we're
		// zero, in which case the distance is infinity.
		self.dist += 1.;

		intersections
	}

	pub fn step_for<'a>(&'a mut self, s: Session<'a>, max_dist: f64) -> RayCastIter<'a> {
		ContextualRayCastIter {
			max_dist,
			back_log: SmallVec::new(),
		}
		.with_context((s, self))
	}
}

#[derive(Debug, Clone)]
pub struct RayCastIntersection {
	_non_exhaustive: (),
	pub block_loc: BlockLocation,
	pub face: BlockFace,
	pub pos: EntityVec,
	pub distance: f64,
}

pub type RayCastIter<'a> = WithContext<(Session<'a>, &'a mut RayCast), ContextualRayCastIter>;

#[derive(Debug, Clone)]
pub struct ContextualRayCastIter {
	pub max_dist: f64,
	back_log: SmallVec<[RayCastIntersection; 3]>,
}

impl<'a> ContextualIter<(Session<'a>, &'a mut RayCast)> for ContextualRayCastIter {
	type Item = RayCastIntersection;

	fn next_on_ref(&mut self, (s, ray): &mut (Session<'a>, &'a mut RayCast)) -> Option<Self::Item> {
		while self.back_log.is_empty() {
			self.back_log = ray.step(*s);
		}

		let next = if !self.back_log.is_empty() {
			self.back_log.remove(0)
		} else if ray.dist() < self.max_dist {
			self.back_log = ray.step(*s);

			// It is possible that the ray needs to travel more than 1 unit to get out of a block.
			// The furthest it can travel is `sqrt(3 * 1^2) ~= 1.7` so we have to call this method
			// at most twice. Also, yes, this handles a zero-vector direction because rays with no
			// direction automatically get infinite distance.
			if self.back_log.is_empty() {
				self.back_log = ray.step(*s);
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
