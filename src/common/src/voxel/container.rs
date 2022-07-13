use std::{cell::Cell, collections::HashMap, fmt, hash};

use crucible_core::array::arr;
use geode::prelude::*;

use super::math::{
	Axis3, BlockFace, BlockPos, BlockPosExt, ChunkPos, Sign, WorldPos, WorldPosExt, CHUNK_VOLUME,
};

use crucible_core::c_enum::ExposesVariants;

#[derive(Debug, Default)]
pub struct VoxelWorldData {
	chunks: HashMap<ChunkPos, Owned<Entity>>,
}

impl VoxelWorldData {
	pub fn add_chunk(
		&mut self,
		s: Session,
		pos: ChunkPos,
		me: Entity,
		chunk: Owned<Entity>,
	) -> Option<Owned<Entity>> {
		let weak_chunk = chunk.weak_copy();
		let chunk_data = chunk.get::<VoxelChunkData>(s);

		// Validate chunk's current world
		if chunk_data.world() == Some(me) {
			return None;
		}

		assert_eq!(chunk_data.world(), None);

		// Replace the old chunk with new chunk
		let replaced = self.chunks.insert(pos, chunk);
		if let Some(replaced) = replaced.as_ref() {
			replaced.get::<VoxelChunkData>(s).world.set(None);
		}

		// Update `chunk_data` state
		chunk_data.world.set(Some(me));
		chunk_data.position.set(pos);

		// Link new chunk to neighbors
		for face in BlockFace::variants() {
			let rel = ChunkPos::from_raw(face.unit());
			let neighbor_pos = pos + rel;
			let neighbor = self.chunks.get(&neighbor_pos).map(|neighbor| neighbor.weak_copy());

			// Link ourselves to the neighboring chunk
			chunk_data.neighbors[face.index()].set(neighbor);

			// Link the neighboring chunk to us
			if let Some(neighbor) = neighbor {
				neighbor.get::<VoxelChunkData>(s).neighbors[face.invert().index()]
					.set(Some(weak_chunk));
			}
		}

		replaced
	}

	pub fn get_chunk(&self, pos: ChunkPos) -> Option<Entity> {
		self.chunks.get(&pos).map(|chunk| chunk.weak_copy())
	}
}

#[derive(Debug)]
pub struct VoxelChunkData {
	world: Cell<Option<Entity>>,
	neighbors: [Cell<Option<Entity>>; BlockFace::COUNT],
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
	pub fn world(&self) -> Option<Entity> {
		self.world.get()
	}

	pub fn neighbor(&self, face: BlockFace) -> Option<Entity> {
		self.neighbors[face as usize].get()
	}

	pub fn pos(&self) -> ChunkPos {
		self.position.get()
	}

	pub fn block_state_of(&self, pos: BlockPos) -> RawBlockState<'_> {
		RawBlockState {
			cell: &self.blocks[pos.to_index()],
		}
	}
}

impl Drop for VoxelChunkData {
	fn drop(&mut self) {
		if let Some(_world) = self.world() {
			// TODO
		}
	}
}

#[derive(Copy, Clone)]
pub struct RawBlockState<'a> {
	/// Format:
	///
	/// ```text
	/// LSB
	/// ---- ---- ~~~~ ~~~~ | ---- ---- | ~~~~ ~~~~ |
	/// Material Data       | Variant   | Light lvl |
	/// (u16)               | (u8)      | (u8)      |
	/// ```
	cell: &'a Cell<u32>,
}

impl fmt::Debug for RawBlockState<'_> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("RawBlockState")
			.field("raw", &self.cell.get())
			.field("material", &self.material())
			.field("variant", &self.variant())
			.field("light_level", &self.light_level())
			.finish()
	}
}

impl RawBlockState<'_> {
	pub fn material(self) -> u16 {
		self.cell.get() as u16
	}

	pub fn variant(self) -> u8 {
		self.cell.get().to_be_bytes()[2]
	}

	pub fn light_level(self) -> u8 {
		self.cell.get().to_be_bytes()[3]
	}

	pub fn set_material(self, id: u16) {
		let value = self.cell.get() - self.material() as u32 + id as u32;
		self.cell.set(value);
	}

	pub fn set_variant(self, variant: u8) {
		let mut bytes = self.cell.get().to_be_bytes();
		bytes[2] = variant;
		self.cell.set(u32::from_be_bytes(bytes))
	}

	pub fn set_light_level(self, light_level: u8) {
		let mut bytes = self.cell.get().to_be_bytes();
		bytes[3] = light_level;
		self.cell.set(u32::from_be_bytes(bytes))
	}
}

#[derive(Debug, Copy, Clone)]
pub struct VoxelPointer {
	chunk_cache: Option<Entity>,
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

				self.chunk_cache = chunk_cache.get::<VoxelChunkData>(s).neighbor(face);
			}
		} else {
			// We've moved too far. Invalidate the chunk cache.
			self.chunk_cache = None;
		}

		self
	}

	pub fn recompute_cache(&mut self, s: Session, world: Entity, world_data: &VoxelWorldData) {
		// Ensure that our cached chunk is actually in the world.
		if let Some(chunk_cache) = self.chunk_cache {
			if chunk_cache.get::<VoxelChunkData>(s).world() != Some(world) {
				self.chunk_cache = None;
			}
		}

		// Try to attach to the world if our chunk cache is stale.
		if self.chunk_cache.is_none() {
			self.chunk_cache = world_data.get_chunk(self.pos.chunk());
		}
	}

	pub fn chunk(
		&mut self,
		s: Session,
		world: Entity,
		world_data: &VoxelWorldData,
	) -> Option<Entity> {
		self.recompute_cache(s, world, world_data);
		self.chunk_cache
	}

	pub fn pos(self) -> WorldPos {
		self.pos
	}
}
