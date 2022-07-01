use std::{cell::Cell, collections::HashMap, hash};

use geode::prelude::*;

use super::math::{chunk_pos_of, Axis3, BlockFace, ChunkPos, Sign, WorldPos};

use crate::polyfill::c_enum::ExposesVariants;

#[derive(Debug)]
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
		let weak_chunk = *chunk;
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
			let rel: ChunkPos = face.unit();
			let neighbor_pos = pos + rel;
			let neighbor = self.chunks.get(&neighbor_pos).map(|neighbor| **neighbor);

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
		self.chunks.get(&pos).map(|chunk| **chunk)
	}
}

#[derive(Debug)]
pub struct VoxelChunkData {
	world: Cell<Option<Entity>>,
	neighbors: [Cell<Option<Entity>>; BlockFace::COUNT],
	position: Cell<ChunkPos>,
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
}

impl Drop for VoxelChunkData {
	fn drop(&mut self) {
		if let Some(_world) = self.world() {
			// TODO
		}
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
		let chunk_pos = chunk_pos_of(pos);
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

	pub fn get_neighbor_with_stride(mut self, s: Session, face: BlockFace, stride: i64) -> Self {
		debug_assert!(stride >= 0);

		// Update position, keeping track of our chunk positions.
		let old_chunk_pos = chunk_pos_of(self.pos);
		self.pos += face.unit() * stride as i64;
		let new_chunk_pos = chunk_pos_of(self.pos);

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
			self.chunk_cache = world_data.get_chunk(chunk_pos_of(self.pos));
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