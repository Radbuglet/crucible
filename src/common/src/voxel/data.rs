use std::{
	collections::HashMap,
	fmt, mem,
	sync::{Arc, Weak},
};

use crucible_core::{
	lang::{lifetime::try_transform_ref, polyfill::OptionPoly},
	mem::{
		array::arr,
		c_enum::{CEnum, CEnumMap},
	},
	prelude::*,
};

use super::math::{BlockFace, BlockVec, BlockVecExt, ChunkVec, CHUNK_VOLUME};

// === Voxel Data Containers === //

pub trait ChunkFactory: fmt::Debug {
	fn create(&mut self, s: &DynSession, world_ent: &Arc<Entity>) -> Arc<Entity>;
}

#[derive(Debug)]
pub struct VoxelWorldData {
	factory: Box<dyn ChunkFactory>,
	chunks: HashMap<ChunkVec, Arc<Entity>>,
	dirty_chunks: Vec<Weak<Entity>>,
}

impl VoxelWorldData {
	pub fn add_chunk(
		&mut self,
		s: &impl Session,
		me: &Arc<Entity>,
		chunk_pos: ChunkVec,
	) -> &Arc<Entity> {
		// Create the chunk
		let chunk = self.factory.create(s.as_dyn(), me);

		// Validate it
		let mut chunk_data = chunk.borrow_mut::<VoxelChunkData>(s);
		debug_assert!(chunk_data.world.is_none());
		debug_assert!(!self.chunks.contains_key(&chunk_pos));

		// Link neighbors
		for face in BlockFace::variants() {
			let rel = face.unit();
			let neighbor_pos = chunk_pos + rel;
			let neighbor = self.chunks.get(&neighbor_pos);

			chunk_data.neighbors[face] = neighbor.map(Arc::downgrade);

			if let Some(neighbor) = neighbor {
				let mut neighbor_chunk = neighbor.borrow_mut::<VoxelChunkData>(s);

				neighbor_chunk.neighbors[face.invert()] = Some(Arc::downgrade(&chunk));
			}
		}

		// Register chunk
		chunk_data.world = Some(Arc::downgrade(me));
		chunk_data.pos = chunk_pos;
		chunk_data.flagged = false;
		drop(chunk_data);

		self.chunks.entry(chunk_pos).or_insert(chunk)
	}

	pub fn remove_chunk(&mut self, s: &impl Session, chunk_pos: ChunkVec) {
		// Get chunk
		let chunk = match self.chunks.remove(&chunk_pos) {
			Some(chunk) => chunk,
			None => {
				log::error!("attempted to remove non-existent chunk at position {chunk_pos:?}");
				return;
			}
		};

		let mut chunk_data = chunk.borrow_mut::<VoxelChunkData>(s);
		chunk_data.world = None;

		// Unlink chunk from neighbors
		for (face, neighbor) in chunk_data.neighbors.iter_mut() {
			if let Some(neighbor) = neighbor {
				let neighbor = neighbor.upgrade().unwrap();
				let mut neighbor_data = neighbor.borrow_mut::<VoxelChunkData>(s);
				neighbor_data.neighbors[face.invert()] = None;
			}

			*neighbor = None;
		}
	}

	pub fn get_chunk(&self, chunk_pos: ChunkVec) -> Option<&Arc<Entity>> {
		self.chunks.get(&chunk_pos)
	}

	pub fn get_or_add_chunk(
		&mut self,
		s: &impl Session,
		me: &Arc<Entity>,
		chunk_pos: ChunkVec,
	) -> &Arc<Entity> {
		match try_transform_ref(self, |this| this.get_chunk(chunk_pos)) {
			Ok(chunk) => chunk,
			Err(this) => this.add_chunk(s, me, chunk_pos),
		}
	}

	pub fn drain_dirty_chunks<'a>(
		&mut self,
		me: &'a Arc<Entity>,
		s: &'a impl Session,
	) -> impl Iterator<Item = Arc<Entity>> + 'a {
		let dirty_chunks = mem::replace(&mut self.dirty_chunks, Vec::new());

		dirty_chunks.into_iter().filter_map(|flagged| {
			let flagged = flagged.upgrade()?;
			let mut flagged_data = flagged.borrow_mut::<VoxelChunkData>(s);
			if !Self::owns_chunk(me, &flagged_data) {
				return None;
			}

			flagged_data.flagged = false;
			drop(flagged_data);

			Some(flagged)
		})
	}

	pub fn owns_chunk(me: &Arc<Entity>, chunk_data: &VoxelChunkData) -> bool {
		chunk_data
			.world
			.as_ref()
			.is_some_and(|world| world.as_ptr() == Arc::as_ptr(me))
	}
}

impl Provider for VoxelWorldData {
	fn provide<'r>(&'r self, demand: &mut Demand<'r>) {
		demand.propose(self);
	}
}

#[derive(Debug)]
pub struct VoxelChunkData {
	world: Option<Weak<Entity>>,
	flagged: bool,
	pos: ChunkVec,
	neighbors: CEnumMap<BlockFace, Option<Weak<Entity>>>,
	blocks: Box<[u32; CHUNK_VOLUME as usize]>,
}

impl VoxelChunkData {
	pub fn world(&self) -> Option<&Weak<Entity>> {
		self.world.as_ref()
	}

	pub fn neighbor(&self, face: BlockFace) -> Option<&Weak<Entity>> {
		self.neighbors[face].as_ref()
	}

	pub fn pos(&self) -> ChunkVec {
		self.pos
	}

	pub fn block_state(&self, pos: BlockVec) -> BlockState {
		BlockState::decode(self.blocks[pos.to_index()])
	}

	pub fn mark_dirty(&mut self, s: &impl Session, me: &Arc<Entity>) {
		if !self.flagged {
			self.flagged = true;

			if let Some(world) = &self.world {
				let world = world.upgrade().unwrap();
				let mut world_data = world.borrow_mut::<VoxelWorldData>(s);
				world_data.dirty_chunks.push(Arc::downgrade(me));
			}
		}
	}

	pub fn set_block_state(
		&mut self,
		s: &impl Session,
		me: &Arc<Entity>,
		pos: BlockVec,
		state: BlockState,
	) {
		if self.block_state(pos) != state {
			self.set_block_state_raw(pos, state);
			self.mark_dirty(s, me);
		}
	}

	pub fn set_block_state_raw(&mut self, pos: BlockVec, state: BlockState) {
		self.blocks[pos.to_index()] = state.encode();
	}
}

impl Default for VoxelChunkData {
	fn default() -> Self {
		Self {
			world: None,
			flagged: false,
			pos: ChunkVec::ZERO,
			neighbors: CEnumMap::default(),
			blocks: Box::new(arr![0; CHUNK_VOLUME as usize]),
		}
	}
}

impl Provider for VoxelChunkData {
	fn provide<'r>(&'r self, demand: &mut Demand<'r>) {
		demand.propose(self);
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
		let variant = word.to_le_bytes()[2];
		let light_level = word.to_le_bytes()[3];

		let decoded = Self {
			material,
			variant,
			light_level,
		};

		debug_assert_eq!(
			word,
			decoded.encode(),
			"Decoding of {word} as {decoded:?} resulted in a different round-trip encoding. This is a bug."
		);

		decoded
	}

	pub fn encode(&self) -> u32 {
		let mut enc = self.material as u32;
		enc += (self.variant as u32) << 17;
		enc += (self.light_level as u32) << 25;
		enc
	}
}
