use std::mem;

use crucible_util::mem::c_enum::{CEnum, CEnumMap};
use geode::{Entity, Storage};
use hashbrown::HashMap;

use super::math::{BlockFace, BlockVec, BlockVecExt, ChunkVec, CHUNK_VOLUME};

// === World === //

#[derive(Debug, Default)]
pub struct VoxelWorldData {
	pos_map: HashMap<ChunkVec, Entity>,
	flagged: Vec<Entity>,
}

impl VoxelWorldData {
	pub fn add_chunk(
		&mut self,
		(chunks,): (&mut Storage<VoxelChunkData>,),
		pos: ChunkVec,
		chunk: Entity,
	) {
		debug_assert!(!self.pos_map.contains_key(&pos));

		// Link to neighbors
		let mut chunk_data = VoxelChunkData {
			pos,
			flagged: None,
			neighbors: CEnumMap::default(),
			data: Box::new([0; CHUNK_VOLUME as usize]),
		};

		for face in BlockFace::variants() {
			let n_pos = pos + face.unit();
			let n_ent = match self.pos_map.get(&n_pos) {
				Some(ent) => *ent,
				None => continue,
			};

			chunk_data.neighbors[face] = Some(n_ent);
			chunks[n_ent].neighbors[face.invert()] = Some(chunk);
		}

		// Create chunk
		self.pos_map.insert(pos, chunk);
		chunks.insert(chunk, chunk_data);
	}

	pub fn get_chunk(&self, pos: ChunkVec) -> Option<Entity> {
		self.pos_map.get(&pos).copied()
	}

	pub fn remove_chunk(&mut self, (chunks,): (&mut Storage<VoxelChunkData>,), pos: ChunkVec) {
		let chunk = self.pos_map.remove(&pos).unwrap();
		let chunk_data = chunks.try_remove(chunk).unwrap();

		// Unlink neighbors
		for (face, &neighbor) in chunk_data.neighbors.iter() {
			let Some(neighbor) = neighbor else {
				continue;
			};
			chunks[neighbor].neighbors[face.invert()] = None;
		}

		// Remove from dirty queue
		if let Some(flagged_idx) = chunk_data.flagged {
			self.flagged.swap_remove(flagged_idx);

			if let Some(moved) = self.flagged.get(flagged_idx).copied() {
				chunks[moved].flagged = Some(flagged_idx);
			}
		}
	}

	pub fn flag_chunk(&mut self, (chunk_data,): (&mut VoxelChunkData,), chunk: Entity) {
		if chunk_data.flagged.is_none() {
			chunk_data.flagged = Some(self.flagged.len());
			self.flagged.push(chunk);
		}
	}

	pub fn flush_flagged(&mut self, (chunks,): (&mut Storage<VoxelChunkData>,)) -> Vec<Entity> {
		let flagged = mem::take(&mut self.flagged);

		for &flagged in &flagged {
			chunks[flagged].flagged = None;
		}

		flagged
	}
}

#[derive(Debug)]
pub struct VoxelChunkData {
	pos: ChunkVec,
	flagged: Option<usize>,
	neighbors: CEnumMap<BlockFace, Option<Entity>>,
	data: Box<[u32; CHUNK_VOLUME as usize]>,
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

	pub fn set_block_state(
		&mut self,
		(world,): (&mut VoxelWorldData,),
		me: Entity,
		pos: BlockVec,
		state: BlockState,
	) {
		let old = &mut self.data[pos.to_index()];
		let new = state.encode();

		if *old != new {
			*old = new;
			world.flag_chunk((self,), me);
		}
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
		enc += (self.variant as u32) << 16;
		enc += (self.light_level as u32) << (16 + 8);
		enc
	}
}
