use std::collections::HashMap;

use crucible_core::{
	ecs::{
		celled::{CelledStorage, CelledStorageView},
		context::Provider,
		core::Entity,
	},
	mem::c_enum::{CEnum, CEnumMap},
};

use super::math::{
	BlockFace, BlockVec, BlockVecExt, ChunkVec, WorldVec, WorldVecExt, CHUNK_VOLUME,
};

// === World === //

pub trait ChunkFactory {
	fn create<C>(&mut self, cx: &mut C, pos: ChunkVec) -> Entity;
}

#[derive(Debug)]
pub struct WorldData {
	pos_map: HashMap<ChunkVec, Entity>,
	flagged: Vec<Entity>,
}

impl WorldData {
	pub fn add_chunk(
		&mut self,
		(chunks,): (&mut CelledStorage<ChunkData>,),
		pos: ChunkVec,
		chunk: Entity,
	) {
		debug_assert!(!self.pos_map.contains_key(&pos));

		// Create chunk
		chunks.add(
			chunk,
			ChunkData {
				pos,
				flagged: None,
				neighbors: CEnumMap::default(),
				data: [0; CHUNK_VOLUME as usize],
			},
		);

		// Link to neighbors
		let data = chunks.borrow_dyn();
		let mut chunk_data = data.borrow_mut(chunk);

		for face in BlockFace::variants() {
			let n_pos = pos + face.unit();
			let n_ent = match self.pos_map.get(&n_pos) {
				Some(ent) => *ent,
				None => continue,
			};
			let mut n_data = data.borrow_mut(n_ent);

			chunk_data.neighbors[face] = Some(n_ent);
			n_data.neighbors[face.invert()] = Some(chunk);
		}
	}

	pub fn get_chunk(&self, pos: ChunkVec) -> Option<Entity> {
		self.pos_map.get(&pos).copied()
	}

	pub fn remove_chunk(&mut self, (chunks,): (&mut CelledStorage<ChunkData>,), pos: ChunkVec) {
		let chunk = self.pos_map[&pos];
		let chunk_data = chunks.remove(chunk).unwrap();

		// Unlink neighbors
		for (face, n_ent) in chunk_data.neighbors.iter() {
			let n_ent = match n_ent {
				Some(ent) => *ent,
				None => continue,
			};
			let n_data = chunks.get_mut(n_ent);

			n_data.neighbors[face.invert()] = None;
		}

		// Remove from dirty queue
		if let Some(flagged_idx) = chunk_data.flagged {
			self.flagged.swap_remove(flagged_idx);

			if let Some(moved) = self.flagged.get(flagged_idx).copied() {
				let moved_data = chunks.get_mut(moved);
				moved_data.flagged = Some(flagged_idx);
			}
		}
	}
}

#[derive(Debug)]
pub struct ChunkData {
	pos: ChunkVec,
	flagged: Option<usize>,
	neighbors: CEnumMap<BlockFace, Option<Entity>>,
	data: [u32; CHUNK_VOLUME as usize],
}

impl ChunkData {
	pub fn pos(&self) -> ChunkVec {
		self.pos
	}

	pub fn neighbor(&self, face: BlockFace) -> Option<Entity> {
		self.neighbors[face]
	}

	pub fn block_state(&self, pos: BlockVec) -> BlockState {
		BlockState::decode(self.data[pos.to_index()])
	}

	pub fn set_block_state(&mut self, pos: BlockVec, state: BlockState) {
		self.data[pos.to_index()] = state.encode();
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

// === Location === //

#[derive(Debug, Copy, Clone)]
pub struct Location {
	pos: WorldVec,
	chunk: Option<Entity>,
}

impl Location {
	pub fn new(world: &WorldData, pos: WorldVec) -> Self {
		Self {
			pos,
			chunk: world.get_chunk(pos.chunk()),
		}
	}

	pub fn refresh(&mut self, (world,): (&WorldData,)) {
		self.chunk = world.get_chunk(self.pos.chunk());
	}

	pub fn move_to_neighbor(
		&mut self,
		(world, chunks): (&WorldData, &CelledStorageView<ChunkData>),
		face: BlockFace,
	) {
		// Update position
		let new_pos = self.pos + face.unit();
		if new_pos.chunk() == self.pos.chunk() {
			return;
		}

		// Update chunk cache
		if let Some(chunk) = self.chunk {
			self.chunk = chunks.borrow(chunk).neighbor(face);
		} else {
			self.refresh((world,));
		}
	}

	pub fn move_by(&mut self, cx: (&WorldData, &CelledStorageView<ChunkData>), delta: WorldVec) {
		self.move_to(cx, self.pos + delta);
	}

	pub fn move_to(
		&mut self,
		(world, chunks): (&WorldData, &CelledStorageView<ChunkData>),
		new_pos: WorldVec,
	) {
		// Update cache
		if let (Some(chunk), Some(face)) = (
			self.chunk,
			BlockFace::from_vec((new_pos.chunk() - self.pos.chunk()).to_glam()),
		) {
			self.chunk = chunks.borrow(chunk).neighbor(face);
		} else {
			self.refresh((world,));
		}

		// Update position
		self.pos = new_pos;
	}

	pub fn state(self, (chunks,): (&CelledStorageView<ChunkData>,)) -> Option<BlockState> {
		self.chunk
			.map(|chunk| chunks.borrow(chunk).block_state(self.pos.block()))
	}

	pub fn set_state(self, (chunks,): (&CelledStorageView<ChunkData>,), state: BlockState) {
		let chunk = match self.chunk {
			Some(chunk) => chunk,
			None => {
				log::warn!("`set_state` called on `BlockLocation` outside of the world.");
				return;
			}
		};

		chunks
			.borrow_mut(chunk)
			.set_block_state(self.pos.block(), state);
	}

	pub fn set_state_or_create(
		self,
		cx: &mut impl Provider,
		factory: &mut impl ChunkFactory,
		state: BlockState,
	) {
		// TODO: implement "rest" destructing
		let (world, chunks, rest) =
			cx.pack::<(&mut WorldData, &mut CelledStorage<ChunkData>, &mut ())>();

		// Fetch chunk
		let chunk = match self.chunk {
			Some(chunk) => chunk,
			None => {
				let pos = self.pos.chunk();
				let chunk = factory.create(rest, pos);
				world.add_chunk((chunks,), pos, chunk);
				chunk
			}
		};

		// Set block state
		chunks
			.get_mut(chunk)
			.set_block_state(self.pos.block(), state);
	}
}
