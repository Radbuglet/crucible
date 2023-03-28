use std::{
	cell::{Ref, RefMut},
	mem,
	ops::{Deref, DerefMut},
};

use bort::{storage, Entity, OwnedEntity, Storage};
use crucible_util::{
	delegate,
	lang::view::View,
	mem::c_enum::{CEnum, CEnumMap},
};
use hashbrown::HashMap;
use typed_glam::traits::{CastVecFrom, SignedNumericVector3};

use crate::{material::AIR_MATERIAL_SLOT, world::math::WorldVecExt};

use super::math::{BlockFace, BlockVec, BlockVecExt, ChunkVec, EntityVec, WorldVec, CHUNK_VOLUME};

// === World === //

delegate! {
	pub fn VoxelChunkFactory(pos: ChunkVec) -> OwnedEntity
}

#[derive(Debug)]
pub struct VoxelWorldData {
	data_store: &'static Storage<VoxelChunkData>,
	chunk_factory: VoxelChunkFactory,
	pos_map: HashMap<ChunkVec, OwnedEntity>,
	flag_list: VoxelWorldFlagList,
}

#[derive(Debug, Default)]
struct VoxelWorldFlagList {
	flagged: Vec<Entity>,
}

impl VoxelWorldFlagList {
	fn add(&mut self, chunk_data: &mut VoxelChunkData, chunk: Entity) {
		if chunk_data.flagged.is_none() {
			chunk_data.flagged = Some(self.flagged.len());
			self.flagged.push(chunk);
		}
	}
}

impl VoxelWorldData {
	pub fn new(chunk_factory: VoxelChunkFactory) -> Self {
		Self {
			data_store: storage(),
			chunk_factory,
			pos_map: HashMap::default(),
			flag_list: VoxelWorldFlagList::default(),
		}
	}

	pub fn try_get_chunk(&self, pos: ChunkVec) -> Option<Entity> {
		self.pos_map.get(&pos).map(OwnedEntity::entity)
	}

	pub fn get_chunk_or_create(&mut self, pos: ChunkVec) -> Entity {
		// Return the chunk if it already exists
		if let Some(chunk) = self.pos_map.get(&pos) {
			return chunk.entity();
		}

		// Register chunk
		let (chunk, chunk_ref) = (self.chunk_factory)(pos).split_guard();
		self.data_store.insert(
			chunk_ref,
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

		// Add the new chunk to the dirty queue
		self.flag_list.add(&mut chunk_data, chunk_ref);

		chunk_ref
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
			self.flag_list.add(&mut self.data, self.chunk);
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
	pub const AIR: Self = Self {
		material: AIR_MATERIAL_SLOT,
		light_level: 0,
		variant: 0,
	};

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

	pub fn is_air(&self) -> bool {
		self.material == AIR_MATERIAL_SLOT
	}

	pub fn is_not_air(&self) -> bool {
		!self.is_air()
	}
}

// === Location === //

pub type BlockLocation = Location<WorldVec>;
pub type EntityLocation = Location<EntityVec>;

#[derive(Debug, Copy, Clone)]
pub struct Location<V> {
	pos: V,
	chunk_cache: Option<Entity>,
}

impl<V> Location<V>
where
	WorldVec: CastVecFrom<V>,
	V: CastVecFrom<WorldVec>,
	V: SignedNumericVector3,
{
	pub fn new(world: &VoxelWorldData, pos: V) -> Self {
		Self {
			pos,
			chunk_cache: world.try_get_chunk(WorldVec::cast_from(pos).chunk()),
		}
	}

	pub fn new_uncached(pos: V) -> Self {
		Self {
			pos,
			chunk_cache: None,
		}
	}

	pub fn refresh(&mut self, world: &VoxelWorldData) {
		self.chunk_cache = world.try_get_chunk(WorldVec::cast_from(self.pos).chunk());
	}

	pub fn pos(&self) -> V {
		self.pos
	}

	pub fn set_pos_within_chunk(&mut self, pos: V) {
		debug_assert_eq!(
			WorldVec::cast_from(pos).chunk(),
			WorldVec::cast_from(self.pos).chunk()
		);

		self.pos = pos;
	}

	pub fn chunk(&mut self, world: &VoxelWorldData) -> Option<Entity> {
		match self.chunk_cache {
			Some(chunk) => Some(chunk),
			None => {
				self.refresh(world);
				self.chunk_cache
			}
		}
	}

	pub fn move_to_neighbor(&mut self, world: &VoxelWorldData, face: BlockFace) {
		// Update position
		let old_pos = self.pos;
		self.pos += face.unit_typed::<V>();

		// Update chunk cache
		if WorldVec::cast_from(old_pos).chunk() != WorldVec::cast_from(self.pos).chunk() {
			if let Some(chunk) = self.chunk_cache {
				self.chunk_cache = world.chunk_state(chunk).neighbor(face);
			} else {
				self.refresh(world);
			}
		}
	}

	pub fn at_neighbor(mut self, world: &VoxelWorldData, face: BlockFace) -> Self {
		self.move_to_neighbor(world, face);
		self
	}

	pub fn move_to(&mut self, world: &VoxelWorldData, new_pos: V) {
		let chunk_delta =
			WorldVec::cast_from(new_pos).chunk() - WorldVec::cast_from(self.pos).chunk();

		if let (Some(chunk), Some(face)) =
			(self.chunk_cache, BlockFace::from_vec(chunk_delta.to_glam()))
		{
			self.pos = new_pos;
			self.chunk_cache = world.chunk_state(chunk).neighbor(face);
		} else {
			self.pos = new_pos;
			self.refresh(world);
		}
	}

	pub fn at_absolute(mut self, world: &VoxelWorldData, new_pos: V) -> Self {
		self.move_to(world, new_pos);
		self
	}

	pub fn move_relative(&mut self, world: &VoxelWorldData, delta: V) {
		self.move_to(world, self.pos + delta);
	}

	pub fn at_relative(mut self, world: &VoxelWorldData, delta: V) -> Self {
		self.move_relative(world, delta);
		self
	}

	pub fn state(&mut self, world: &VoxelWorldData) -> Option<BlockState> {
		self.chunk(world).map(|chunk| {
			world
				.chunk_state(chunk)
				.block_state(WorldVec::cast_from(self.pos).block())
		})
	}

	pub fn set_state_in_world(&mut self, world: &mut VoxelWorldData, state: BlockState) {
		let chunk = match self.chunk(world) {
			Some(chunk) => chunk,
			None => {
				log::warn!("`set_state` called on `BlockLocation` outside of the world.");
				return;
			}
		};

		world
			.chunk_state_mut(chunk)
			.set_block_state(WorldVec::cast_from(self.pos).block(), state);
	}

	pub fn set_state_or_create(&mut self, world: &mut VoxelWorldData, state: BlockState) {
		// Fetch chunk
		let chunk = match self.chunk(world) {
			Some(chunk) => chunk,
			None => {
				let pos = WorldVec::cast_from(self.pos).chunk();
				world.get_chunk_or_create(pos)
			}
		};

		// Set block state
		world
			.chunk_state_mut(chunk)
			.set_block_state(WorldVec::cast_from(self.pos).block(), state);
	}

	pub fn as_block_location(&self) -> BlockLocation {
		BlockLocation {
			chunk_cache: self.chunk_cache,
			pos: WorldVec::cast_from(self.pos),
		}
	}
}
