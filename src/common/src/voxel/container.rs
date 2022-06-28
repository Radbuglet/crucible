use std::{cell::Cell, collections::HashMap};

use geode::prelude::*;

use super::math::{BlockFace, ChunkPos};

use crate::polyfill::c_enum::ExposesVariants;

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
}

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
