use std::{
	cell::{Ref, RefMut},
	mem,
	ops::Deref,
};

use bort::{Obj, OwnedObj};
use crucible_util::mem::{array::boxed_arr_from_fn, c_enum::CEnumMap, hash::FxHashMap};
use typed_glam::traits::{CastVecFrom, SignedNumericVector3};

use crate::{
	material::MaterialId,
	math::{
		BlockFace, BlockVec, BlockVecExt, ChunkVec, EntityVec, WorldVec, WorldVecExt, CHUNK_VOLUME,
	},
};

// === WorldVoxelData === //

#[derive(Debug, Default)]
pub struct WorldVoxelData {
	// A map from chunk positions to chunk instances. We assume that these objects exhibit exterior
	// mutability.
	pos_map: FxHashMap<ChunkVec, OwnedObj<ChunkVoxelData>>,

	// An unordered vector of all chunks that have been updated since the last flush. These never
	// dangle.
	dirty: Vec<Obj<ChunkVoxelData>>,
}

impl WorldVoxelData {
	// === Core Methods === //

	pub fn insert_chunk(
		&mut self,
		pos: ChunkVec,
		chunk: OwnedObj<ChunkVoxelData>,
	) -> Option<OwnedObj<ChunkVoxelData>> {
		// Insert the new chunk into the map and unregister the old one.
		let (chunk_guard, chunk) = chunk.split_guard();
		let old = self.pos_map.insert(pos, chunk_guard);

		if let Some(old) = &old {
			// While, yes, we could reuse the old chunk's neighbor list and dirty list index to make
			// replacing current chunks more efficient, replacements in insertion are sufficiently
			// rare that we don't bother to optimize for it.
			self.internal_unlink(&mut old.get_mut());
		}

		// Set the chunk's main state
		let mut chunk_state = chunk.get_mut();
		chunk_state.pos = pos;

		// Link the chunk to its neighbors
		for (face, neighbor) in chunk_state.neighbors.iter_mut() {
			let neighbor_chunk = self.get_chunk(pos + face.unit_typed::<ChunkVec>());
			*neighbor = neighbor_chunk;

			if let Some(neighbor_chunk) = neighbor_chunk {
				neighbor_chunk.get_mut().neighbors[face.invert()] = Some(chunk);
			}
		}

		// Add the chunk to the dirty queue
		chunk_state.dirty_index = self.dirty.len();
		self.dirty.push(chunk);

		old
	}

	pub fn remove_chunk(&mut self, pos: ChunkVec) -> Option<OwnedObj<ChunkVoxelData>> {
		let chunk = self.pos_map.remove(&pos);

		if let Some(chunk) = &chunk {
			self.internal_unlink(&mut chunk.get_mut());
		}

		chunk
	}

	pub fn get_chunk(&self, pos: ChunkVec) -> Option<Obj<ChunkVoxelData>> {
		self.pos_map.get(&pos).map(OwnedObj::obj)
	}

	pub fn read_chunk(&self, chunk: Obj<ChunkVoxelData>) -> Ref<'_, ChunkVoxelData> {
		chunk.get()
	}

	pub fn write_chunk(&mut self, chunk: Obj<ChunkVoxelData>) -> ChunkVoxelDataMut<'_> {
		ChunkVoxelDataMut {
			world: self,
			chunk,
			chunk_state: chunk.get_mut(),
		}
	}

	pub fn flush_dirty(&mut self) -> Vec<Obj<ChunkVoxelData>> {
		let dirty = mem::take(&mut self.dirty);

		for &dirty in &dirty {
			dirty.get_mut().dirty_index = usize::MAX;
		}

		dirty
	}

	// === Internal methods === //

	fn internal_mark_dirty(&mut self, chunk: Obj<ChunkVoxelData>, dirty_index: &mut usize) {
		if *dirty_index == usize::MAX {
			*dirty_index = self.dirty.len();
			self.dirty.push(chunk);
		}
	}

	fn internal_unlink(&mut self, chunk_state: &mut ChunkVoxelData) {
		// Unlink from neighbors
		for (face, neighbor) in chunk_state.neighbors.iter() {
			if let Some(neighbor) = neighbor {
				neighbor.get_mut().neighbors[face.invert()] = None;
			}
		}

		// Remove from dirty queue
		if chunk_state.dirty_index != usize::MAX {
			self.dirty.swap_remove(chunk_state.dirty_index);

			if let Some(displaced) = self.dirty.get(chunk_state.dirty_index) {
				displaced.get_mut().dirty_index = chunk_state.dirty_index;
			}
		}
	}
}

#[derive(Debug, Default)]
pub struct ChunkVoxelData {
	// The position of the chunk in the world.
	pos: ChunkVec,

	// References to the chunk's neighbors. These references never dangle.
	neighbors: CEnumMap<BlockFace, Option<Obj<Self>>>,

	// The chunk's block states or `None` if the chunk hasn't loaded yet.
	blocks: Option<Box<[Block; CHUNK_VOLUME as usize]>>,

	// The index of the chunk in the dirty queue or `usize::MAX` if it isn't in the queue.
	dirty_index: usize,
}

impl ChunkVoxelData {
	pub fn pos(&self) -> ChunkVec {
		self.pos
	}

	pub fn neighbor(&self, face: BlockFace) -> Option<Obj<Self>> {
		self.neighbors[face]
	}

	pub fn blocks(&self) -> Option<ChunkBlocks<'_>> {
		self.blocks.as_ref().map(|blocks| ChunkBlocks(blocks))
	}

	pub fn block(&self, pos: BlockVec) -> Option<Block> {
		self.blocks().map(|blocks| blocks.block(pos))
	}

	pub fn block_or_air(&self, pos: BlockVec) -> Block {
		self.block(pos).unwrap_or(Block::AIR)
	}

	pub fn is_dirty(&self) -> bool {
		self.dirty_index != usize::MAX
	}
}

#[derive(Debug, Copy, Clone)]
pub struct ChunkBlocks<'a>(&'a [Block; CHUNK_VOLUME as usize]);

impl ChunkBlocks<'_> {
	pub fn block(&self, pos: BlockVec) -> Block {
		self.0[pos.to_index()]
	}
}

#[derive(Debug)]
pub struct ChunkVoxelDataMut<'a> {
	world: &'a mut WorldVoxelData,
	chunk: Obj<ChunkVoxelData>,
	chunk_state: RefMut<'a, ChunkVoxelData>,
}

impl Deref for ChunkVoxelDataMut<'_> {
	type Target = ChunkVoxelData;

	fn deref(&self) -> &Self::Target {
		&self.chunk_state
	}
}

impl ChunkVoxelDataMut<'_> {
	pub fn mark_dirty(&mut self) {
		self.world
			.internal_mark_dirty(self.chunk, &mut self.chunk_state.dirty_index);
	}

	pub fn load_blocks(&mut self, data: Option<Box<[Block; CHUNK_VOLUME as usize]>>) {
		self.chunk_state.blocks = Some(data.unwrap_or_else(|| boxed_arr_from_fn(|| Block::AIR)));
	}

	pub fn blocks_mut(&mut self) -> Option<VoxelBlocksMut<'_>> {
		let world = &mut *self.world;
		let chunk = self.chunk;
		let chunk_state = &mut *self.chunk_state;

		let blocks = &mut chunk_state.blocks;
		let dirty_index = &mut chunk_state.dirty_index;

		blocks.as_mut().map(|blocks| VoxelBlocksMut {
			world,
			chunk,
			blocks,
			dirty_index,
		})
	}

	#[must_use]
	pub fn try_set_block(&mut self, pos: BlockVec, block: Block) -> bool {
		if let Some(mut blocks) = self.blocks_mut() {
			blocks.set_block(pos, block);
			true
		} else {
			false
		}
	}
}

#[derive(Debug)]
pub struct VoxelBlocksMut<'a> {
	world: &'a mut WorldVoxelData,
	chunk: Obj<ChunkVoxelData>,
	blocks: &'a mut [Block; CHUNK_VOLUME as usize],
	dirty_index: &'a mut usize,
}

impl VoxelBlocksMut<'_> {
	pub fn as_ref(&self) -> ChunkBlocks<'_> {
		ChunkBlocks(self.blocks)
	}

	pub fn block(&self, pos: BlockVec) -> Block {
		self.blocks[pos.to_index()]
	}

	pub fn set_block(&mut self, pos: BlockVec, block: Block) {
		let block_ref = &mut self.blocks[pos.to_index()];

		if *block_ref != block {
			*block_ref = block;
			self.world.internal_mark_dirty(self.chunk, self.dirty_index);
		}
	}
}

// === Block === //

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct Block {
	pub material: MaterialId,
	pub variant: u8,
	pub light: u8,
}

impl Block {
	pub const AIR: Self = Self::new(MaterialId::AIR);

	pub const fn new(material: MaterialId) -> Self {
		Self {
			material,
			variant: 0,
			light: u8::MAX,
		}
	}

	pub fn is_air(&self) -> bool {
		self.material == MaterialId::AIR
	}

	pub fn is_not_air(&self) -> bool {
		!self.is_air()
	}
}

// === VoxelPointer === //

pub type BlockVoxelPointer = VoxelPointer<WorldVec>;
pub type EntityVoxelPointer = VoxelPointer<EntityVec>;

#[derive(Debug, Copy, Clone)]
pub struct VoxelPointer<V> {
	cache: Option<Obj<ChunkVoxelData>>,
	pos: V,
}

impl<V> VoxelPointer<V>
where
	WorldVec: CastVecFrom<V>,
	V: CastVecFrom<WorldVec>,
	V: SignedNumericVector3,
{
	// === Constructors === //

	pub fn new(world: &WorldVoxelData, pos: V) -> Self {
		Self {
			cache: world.get_chunk(WorldVec::cast_from(pos).chunk()),
			pos,
		}
	}

	pub fn new_uncached(pos: V) -> Self {
		Self { cache: None, pos }
	}

	// === Getters === //

	pub fn cached_chunk(&self) -> Option<Obj<ChunkVoxelData>> {
		self.cache
	}

	pub fn pos(&self) -> V {
		self.pos
	}

	pub fn voxel_pos(&self) -> WorldVec {
		self.pos.cast()
	}

	// === Cache management === //

	pub fn refresh(&mut self, world: &WorldVoxelData) {
		self.cache = world.get_chunk(self.voxel_pos().chunk());
	}

	pub fn chunk(&mut self, world: &WorldVoxelData) -> Option<Obj<ChunkVoxelData>> {
		match self.cache {
			Some(cache) if cache.is_alive() => Some(cache),
			_ => {
				self.refresh(world);
				self.cached_chunk()
			}
		}
	}

	pub fn chunk_no_writeback(&self, world: &WorldVoxelData) -> Option<Obj<ChunkVoxelData>> {
		self.clone().chunk(world)
	}

	// === Setters === //

	pub fn set_pos(&mut self, world: Option<&WorldVoxelData>, pos: V) {
		// Update `pos` and determine chunk delta
		let old_chunk = self.voxel_pos().chunk();
		self.pos = pos;
		let new_chunk = self.voxel_pos().chunk();

		// If the chunk changed and we have a cache, update it.
		if let Some(cache) = self.cache.filter(|_| old_chunk != new_chunk) {
			if let (Some(world), Some(face)) = (
				world,
				BlockFace::from_vec((new_chunk - old_chunk).to_glam()),
			) {
				self.cache = world.read_chunk(cache).neighbor(face);
			} else {
				self.cache = None;
			}
		}
	}

	pub fn move_by(&mut self, world: Option<&WorldVoxelData>, rel: V) {
		self.set_pos(world, self.pos + rel);
	}

	pub fn move_to_neighbor(&mut self, world: Option<&WorldVoxelData>, face: BlockFace) {
		self.move_by(world, face.unit_typed());
	}

	// === Operations === //

	pub fn at_absolute(&self, world: Option<&WorldVoxelData>, pos: V) -> Self {
		let mut clone = self.clone();
		clone.set_pos(world, pos);
		clone
	}

	pub fn at_relative(&self, world: Option<&WorldVoxelData>, rel: V) -> Self {
		let mut clone = self.clone();
		clone.move_by(world, rel);
		clone
	}

	pub fn at_neighbor(&self, world: Option<&WorldVoxelData>, face: BlockFace) -> Self {
		let mut clone = self.clone();
		clone.move_to_neighbor(world, face);
		clone
	}

	// === Aliases === //

	pub fn state(&mut self, world: &WorldVoxelData) -> Option<Block> {
		let chunk = self.chunk(world)?;
		world.read_chunk(chunk).block(self.voxel_pos().block())
	}

	#[must_use]
	pub fn try_set_state(&mut self, world: &mut WorldVoxelData, block: Block) -> bool {
		if let Some(chunk) = self.chunk(world) {
			world
				.write_chunk(chunk)
				.try_set_block(self.voxel_pos().block(), block)
		} else {
			false
		}
	}
}

impl EntityVoxelPointer {
	pub fn as_block_location(&self) -> BlockVoxelPointer {
		BlockVoxelPointer {
			cache: self.cache,
			pos: self.voxel_pos(),
		}
	}
}
