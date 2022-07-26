use std::{cell::Cell, collections::HashMap, hash};

use crucible_core::{array::arr, c_enum::ExposesVariants};
use geode::prelude::*;
use smallvec::SmallVec;
use typed_glam::glam::DVec3;

use crate::voxel::math::Line3;

use super::math::{
	Axis3, BlockFace, BlockPos, BlockPosExt, ChunkPos, EntityPos, EntityPosExt, Sign, WorldPos,
	WorldPosExt, CHUNK_VOLUME,
};

// === Voxel Data Containers === //

type WorldEntity = EntityWithRw<VoxelWorldData>;
type ChunkEntity = EntityWith<VoxelChunkData>;

pub type ChunkFactory = dyn Factory<ChunkFactoryRequest, Owned<ChunkEntity>>;

#[derive(Debug)]
pub struct ChunkFactoryRequest {
	pub world: WorldEntity,
}

pub struct VoxelWorldData {
	me: WorldEntity,
	chunk_factory: MaybeOwned<Obj<ChunkFactory>>,
	chunks: HashMap<ChunkPos, Owned<ChunkEntity>>,
}

impl VoxelWorldData {
	pub fn new(world: Entity, chunk_factory: MaybeOwned<Obj<ChunkFactory>>) -> Self {
		Self {
			me: WorldEntity::force_cast(world),
			chunk_factory,
			chunks: Default::default(),
		}
	}

	pub fn entity(&self) -> WorldEntity {
		self.me
	}

	pub fn add_chunk(
		&mut self,
		s: Session,
		pos: ChunkPos,
	) -> (ChunkEntity, Option<Owned<ChunkEntity>>) {
		// Create chunk
		let (chunk_guard, chunk) = self
			.chunk_factory
			.get(s)
			.create(s, ChunkFactoryRequest { world: self.me })
			.to_guard_ref_pair();

		let chunk_data = chunk_guard.comp(s);
		assert_eq!(chunk_data.world(), None);

		// Replace the old chunk with new chunk
		let replaced = self.chunks.insert(pos, chunk_guard);
		if let Some(replaced) = replaced.as_ref() {
			replaced.comp(s).world.set(None);
		}

		// Update `chunk_data` state
		chunk_data.world.set(Some(self.me));
		chunk_data.position.set(pos);

		// Link new chunk to neighbors
		for face in BlockFace::variants() {
			let rel = ChunkPos::from_raw(face.unit());
			let neighbor_pos = pos + rel;
			let neighbor = self
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

	pub fn get_chunk(&self, pos: ChunkPos) -> Option<ChunkEntity> {
		self.chunks.get(&pos).map(|chunk| chunk.weak_copy())
	}

	pub fn get_chunk_or_add(&mut self, s: Session, pos: ChunkPos) -> ChunkEntity {
		if let Some(chunk) = self.get_chunk(pos) {
			chunk
		} else {
			self.add_chunk(s, pos).0
		}
	}
}

#[derive(Debug)]
pub struct VoxelChunkData {
	world: Cell<Option<WorldEntity>>,
	neighbors: [Cell<Option<ChunkEntity>>; BlockFace::COUNT],
	position: Cell<ChunkPos>,
	blocks: Box<[Cell<u32>; CHUNK_VOLUME as usize]>,
}

impl Default for VoxelChunkData {
	fn default() -> Self {
		Self {
			world: Default::default(),
			neighbors: Default::default(),
			position: Default::default(),
			blocks: Box::new(arr![Cell::new(0); CHUNK_VOLUME as usize]),
		}
	}
}

impl VoxelChunkData {
	pub fn world(&self) -> Option<WorldEntity> {
		self.world.get()
	}

	pub fn neighbor(&self, face: BlockFace) -> Option<ChunkEntity> {
		self.neighbors[face as usize].get()
	}

	pub fn pos(&self) -> ChunkPos {
		self.position.get()
	}

	pub fn get_block(&self, pos: BlockPos) -> BlockState {
		BlockState::decode(self.blocks[pos.to_index()].get())
	}

	pub fn set_block(&self, pos: BlockPos, state: BlockState) {
		self.blocks[pos.to_index()].set(state.encode())
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

// === Voxel Pointer === //

#[derive(Debug, Copy, Clone)]
pub struct VoxelPointer {
	chunk_cache: Option<ChunkEntity>,
	pos: WorldPos,
}

impl hash::Hash for VoxelPointer {
	fn hash<H: hash::Hasher>(&self, state: &mut H) {
		self.pos.hash(state);
	}
}

impl Eq for VoxelPointer {}

impl PartialEq for VoxelPointer {
	fn eq(&self, other: &Self) -> bool {
		self.pos == other.pos
	}
}

impl VoxelPointer {
	pub fn new_cached(world: &VoxelWorldData, pos: WorldPos) -> Self {
		let chunk_pos = pos.chunk();
		let chunk_cache = world.get_chunk(chunk_pos);
		Self { chunk_cache, pos }
	}

	pub fn new_uncached(pos: WorldPos) -> Self {
		Self {
			chunk_cache: None,
			pos,
		}
	}

	pub fn get_absolute(self, s: Session, pos: WorldPos) -> Self {
		self.get_relative(s, pos - self.pos)
	}

	pub fn get_relative(mut self, s: Session, delta: WorldPos) -> Self {
		for axis in Axis3::variants() {
			if let Some(sign) = Sign::of(delta[axis]) {
				self = self.get_neighbor_with_stride(
					s,
					BlockFace::compose(axis, sign),
					delta[axis].abs(),
				);
			}
		}
		self
	}

	pub fn get_neighbor(self, s: Session, face: BlockFace) -> Self {
		self.get_neighbor_with_stride(s, face, 1)
	}

	pub fn get_neighbor_with_stride(mut self, s: Session, face: BlockFace, stride: i32) -> Self {
		debug_assert!(stride >= 0);

		// Update position, keeping track of our chunk positions.
		let old_chunk_pos = self.pos.chunk();
		self.pos += face.unit() * stride;
		let new_chunk_pos = self.pos.chunk();

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

	pub fn invalidate_stale_cache(&mut self, s: Session, world_data: &VoxelWorldData) {
		// Ensure that our cached chunk is actually in the world.
		if let Some(chunk_cache) = self.chunk_cache {
			if chunk_cache.comp(s).world() != Some(world_data.entity()) {
				self.chunk_cache = None;
			}
		}
	}

	pub fn recompute_cache(&mut self, s: Session, world_data: &VoxelWorldData) {
		self.invalidate_stale_cache(s, world_data);

		if self.chunk_cache.is_none() {
			self.chunk_cache = world_data.get_chunk(self.pos.chunk());
		}
	}

	pub fn recompute_cache_or_add(&mut self, s: Session, world_data: &mut VoxelWorldData) {
		self.invalidate_stale_cache(s, world_data);

		if self.chunk_cache.is_none() {
			self.chunk_cache = Some(world_data.get_chunk_or_add(s, self.pos.chunk()));
		}
	}

	pub fn chunk(&mut self, s: Session, world_data: &VoxelWorldData) -> Option<ChunkEntity> {
		self.recompute_cache(s, world_data);
		self.chunk_cache
	}

	pub fn chunk_or_add(&mut self, s: Session, world_data: &mut VoxelWorldData) -> ChunkEntity {
		self.recompute_cache_or_add(s, world_data);
		self.chunk_cache.unwrap()
	}

	pub fn pos(self) -> WorldPos {
		self.pos
	}
}

// === Voxel Ray Cast === //

pub struct VoxelRayCast {
	pointer: VoxelPointer,
	pos: EntityPos,
	direction: DVec3,
	distance: f64,
}

impl VoxelRayCast {
	pub fn new(world: &VoxelWorldData, origin: EntityPos, direction: DVec3) -> Self {
		debug_assert!(direction.is_normalized());

		Self {
			pointer: VoxelPointer::new_cached(world, origin.world_pos()),
			pos: origin,
			direction,
			distance: 0.,
		}
	}

	pub fn step(&mut self, s: Session) -> SmallVec<[RayCastIntersection; 3]> {
		let mut local_intersections = SmallVec::new();

		// Compute step info
		let step = Line3 {
			start: self.pos,
			end: self.pos + self.direction,
		};
		let start_block = step.start.world_pos();
		let end_block = step.end.world_pos();
		let block_delta = end_block - start_block;

		// Handle block step
		for axis in Axis3::variants() {
			let axis_delta = block_delta[axis];
			debug_assert!((-1..=1).contains(&axis_delta));

			let crossing_face = match Sign::of(axis_delta) {
				Some(sign) => BlockFace::compose(axis, sign),
				None => continue, // No special handling if we haven't breached the block barrier.
			};

			// Find intersection
			let crossed_layer_depth = self.pointer.pos().block_face_layer(crossing_face);
			let (percent, pos) = axis.aabb_intersect(crossed_layer_depth, step);
			debug_assert!(percent.abs() <= 1.);

			local_intersections.push(RayCastIntersection {
				pos,
				axis,
				distance: self.distance + step.start.distance(pos),
			});

			// Update pointer
			self.pointer = self.pointer.get_neighbor(s, crossing_face);
		}

		// Update positional info
		self.pos += self.direction;
		self.distance += 1.;

		local_intersections
	}
}

#[derive(Debug, Clone)]
pub struct RayCastIntersection {
	pub pos: EntityPos,
	pub axis: Axis3,
	pub distance: f64,
}
