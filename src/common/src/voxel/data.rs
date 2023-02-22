use std::{
	cell::{Ref, RefMut},
	mem,
	ops::{Deref, DerefMut},
};

use bort::{storage, Entity, OwnedEntity, Storage};
use crucible_util::{
	lang::view::View,
	mem::c_enum::{CEnum, CEnumMap},
};
use hashbrown::HashMap;

use super::math::{BlockFace, BlockVec, BlockVecExt, ChunkVec, CHUNK_VOLUME};

// === World === //

#[derive(Debug)]
pub struct VoxelWorldData {
	data_store: &'static Storage<VoxelChunkData>,
	pos_map: HashMap<ChunkVec, OwnedEntity>,
	flag_list: VoxelWorldFlagList,
}

#[derive(Debug, Default)]
struct VoxelWorldFlagList {
	flagged: Vec<Entity>,
}

impl VoxelWorldFlagList {
	fn flag_chunk(&mut self, chunk_data: &mut VoxelChunkData, chunk: Entity) {
		if chunk_data.flagged.is_none() {
			chunk_data.flagged = Some(self.flagged.len());
			self.flagged.push(chunk);
		}
	}
}

impl Default for VoxelWorldData {
	fn default() -> Self {
		Self {
			data_store: storage::<VoxelChunkData>(),
			pos_map: Default::default(),
			flag_list: Default::default(),
		}
	}
}

impl VoxelWorldData {
	pub fn add_chunk(&mut self, pos: ChunkVec, chunk: OwnedEntity) {
		debug_assert!(!self.pos_map.contains_key(&pos));

		// Register chunk
		let (chunk, chunk_ref) = chunk.split_guard();
		self.data_store.insert(
			chunk.entity(),
			VoxelChunkData {
				pos,
				flagged: None,
				neighbors: CEnumMap::default(),
				data: Box::new([0; CHUNK_VOLUME as usize]),
			},
		);
		self.pos_map.insert(pos, chunk);

		// Link to neighbors
		let mut chunk_data = chunk_ref.get_mut::<VoxelChunkData>();

		for face in BlockFace::variants() {
			let neighbor_pos = pos + face.unit();
			let neighbor = match self.pos_map.get(&neighbor_pos) {
				Some(ent) => ent.entity(),
				None => continue,
			};

			chunk_data.neighbors[face] = Some(neighbor);
			neighbor.get_mut::<VoxelChunkData>().neighbors[face.invert()] = Some(chunk_ref);
		}
	}

	pub fn get_chunk(&self, pos: ChunkVec) -> Option<Entity> {
		self.pos_map.get(&pos).map(OwnedEntity::entity)
	}

	pub fn remove_chunk(&mut self, pos: ChunkVec) {
		let chunk = self.pos_map.remove(&pos).unwrap();
		let chunk_data = self.data_store.remove(chunk.entity()).unwrap();

		// Unlink neighbors
		for (face, &neighbor) in chunk_data.neighbors.iter() {
			let Some(neighbor) = neighbor else {
				continue;
			};
			self.data_store.get_mut(neighbor).neighbors[face.invert()] = None;
		}

		// Remove from dirty queue
		if let Some(flagged_idx) = chunk_data.flagged {
			self.flag_list.flagged.swap_remove(flagged_idx);

			if let Some(moved) = self.flag_list.flagged.get(flagged_idx).copied() {
				self.data_store.get_mut(moved).flagged = Some(flagged_idx);
			}
		}
	}

	pub fn chunk_state(&self, chunk: Entity) -> Ref<VoxelChunkDataView> {
		Ref::map(self.data_store.get(chunk), View::from_ref)
	}

	pub fn chunk_state_mut(&mut self, chunk: Entity) -> VoxelChunkDataViewMut {
		VoxelChunkDataViewMut {
			chunk,
			flag_list: &mut self.flag_list,
			data: self.data_store.get_mut(chunk),
		}
	}

	pub fn flush_flagged(&mut self) -> Vec<Entity> {
		let flagged = mem::take(&mut self.flag_list.flagged);

		for &flagged in &flagged {
			flagged.get_mut::<VoxelChunkData>().flagged = None;
		}

		flagged
	}
}

use sealed::VoxelChunkData;
mod sealed {
	use super::*;

	#[derive(Debug)]
	pub struct VoxelChunkData {
		pub(super) pos: ChunkVec,
		pub(super) flagged: Option<usize>,
		pub(super) neighbors: CEnumMap<BlockFace, Option<Entity>>,
		pub(super) data: Box<[u32; CHUNK_VOLUME as usize]>,
	}
}

impl VoxelChunkData {
	pub fn pos(&self) -> ChunkVec {
		self.pos
	}

	pub fn neighbor(&self, face: BlockFace) -> Option<Entity> {
		self.neighbors[face]
	}

	pub fn block_state(&self, pos: BlockVec) -> BlockState {
		BlockState::decode(self.data[pos.to_index()])
	}
}

pub type VoxelChunkDataView = View<VoxelChunkData>;

pub struct VoxelChunkDataViewMut<'a> {
	chunk: Entity,
	flag_list: &'a mut VoxelWorldFlagList,
	data: RefMut<'a, VoxelChunkData>,
}

impl Deref for VoxelChunkDataViewMut<'_> {
	type Target = VoxelChunkData;

	fn deref(&self) -> &Self::Target {
		&self.data
	}
}

impl DerefMut for VoxelChunkDataViewMut<'_> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.data
	}
}

impl VoxelChunkDataViewMut<'_> {
	pub fn set_block_state(&mut self, pos: BlockVec, new: BlockState) {
		let state = &mut self.data.data[pos.to_index()];
		let new = new.encode();

		if *state != new {
			*state = new;
			self.flag_list.flag_chunk(&mut self.data, self.chunk);
		}
	}
}

// === Block State Manipulation === //

pub const AIR_MATERIAL_SLOT: u16 = 0;

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
		enc += (self.variant as u32) << 16;
		enc += (self.light_level as u32) << (16 + 8);
		enc
	}
}
