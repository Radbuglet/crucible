use crate::voxel::coord::{BlockFace, BlockPos, ChunkPos, CHUNK_VOLUME};
use crucible_core::foundation::prelude::*;
use crucible_core::util::meta_enum::EnumMeta;
use std::collections::HashMap;

#[derive(Default)]
pub struct VoxelWorld {
	chunk_store: Storage<VoxelChunk>,
	pos_map: HashMap<ChunkPos, Entity>,
}

impl VoxelWorld {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn add(
		&mut self,
		world: &World,
		pos: ChunkPos,
		entity: Entity,
	) -> Option<(Entity, VoxelChunk)> {
		let replaced = self.pos_map.insert(pos, entity);
		let replaced = replaced.map(|entity| (entity, self.chunk_store.remove(entity).unwrap()));

		// TODO: We should modify the `VoxelChunk` in the `Storage` directly first to avoid a memcpy
		// ^ this is blocked on tracked storages.
		let mut handle = VoxelChunk::new(pos);

		for face in BlockFace::variants() {
			let neighbor_pos = pos + face.unit();
			if let Some(mut neighbor) = self.get_chunk_mut_at(world, neighbor_pos) {
				handle.neighbors[face.index()] = Some(neighbor.entity());
				neighbor.neighbors[face.inverse.index()] = Some(neighbor.entity());
			}
		}

		self.chunk_store.insert(world, entity, handle);
		replaced
	}

	pub fn remove_pos(&mut self, world: &World, pos: ChunkPos) {
		if let Some(chunk) = self.pos_map.remove(&pos) {
			let handle = self.chunk_store.remove(chunk).unwrap();
			for face in BlockFace::variants() {
				let neighbor = handle.neighbors[face.index()]
					.and_then(|neighbor| self.chunk_store.try_get_mut(world, neighbor));

				if let Some(mut neighbor) = neighbor {
					neighbor.neighbors[face.inverse.index()] = None;
				}
			}
		}
	}

	pub fn chunks(&self) -> &Storage<VoxelChunk> {
		&self.chunk_store
	}

	pub fn get_chunk_at(&self, world: &World, pos: ChunkPos) -> Option<ComponentPair<VoxelChunk>> {
		self.pos_map
			.get(&pos)
			.copied()
			.and_then(|entity| self.chunk_store.try_get(world, entity))
	}

	pub fn get_chunk_mut_at(
		&mut self,
		world: &World,
		pos: ChunkPos,
	) -> Option<ComponentPairMut<VoxelChunk>> {
		self.pos_map
			.get(&pos)
			.copied()
			.and_then(|entity| self.chunk_store.try_get_mut(world, entity))
	}

	pub fn get_chunk(&self, world: &World, id: Entity) -> Option<ComponentPair<VoxelChunk>> {
		self.chunk_store.try_get(world, id)
	}

	pub fn get_chunk_mut(
		&mut self,
		world: &World,
		id: Entity,
	) -> Option<ComponentPairMut<VoxelChunk>> {
		self.chunk_store.try_get_mut(world, id)
	}
}

#[derive(Debug)]
pub struct VoxelChunk {
	pos: ChunkPos,
	neighbors: [Option<Entity>; BlockFace::COUNT],
	data: [u16; CHUNK_VOLUME as usize],
}

impl VoxelChunk {
	pub fn new(pos: ChunkPos) -> Self {
		Self {
			pos,
			neighbors: Default::default(),
			data: [0; CHUNK_VOLUME as usize],
		}
	}

	pub fn pos(&self) -> ChunkPos {
		self.pos
	}

	pub fn get_block(&self, pos: BlockPos) -> u16 {
		self.data[pos.to_index()]
	}

	pub fn set_block(&mut self, pos: BlockPos, data: u16) {
		self.data[pos.to_index()] = data;
	}

	pub fn blocks(&self) -> impl Iterator<Item = (BlockPos, u16)> + '_ {
		self.data
			.iter()
			.copied()
			.enumerate()
			.map(|(index, data)| (BlockPos::from_index(index), data))
	}
}
