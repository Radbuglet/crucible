use std::{mem, ops::Deref};

use bort::{cx, CompMut, CompRef, Cx, Obj, OwnedObj};
use crucible_util::mem::{array::boxed_arr_from_fn, c_enum::CEnumMap, hash::FxHashMap};
use typed_glam::traits::{CastVecFrom, SignedNumericVector3};

use crate::{
	material::{MaterialId, MaterialInfo, MaterialMarker, MaterialRegistry},
	math::{
		BlockFace, BlockVec, BlockVecExt, ChunkVec, EntityVec, WorldVec, WorldVecExt, CHUNK_VOLUME,
	},
};

// === Materials === //

#[non_exhaustive]
pub struct BlockMaterialMarker;

impl MaterialMarker for BlockMaterialMarker {}

pub type BlockMaterialRegistry = MaterialRegistry<BlockMaterialMarker>;
pub type BlockMaterialInfo = MaterialInfo<BlockMaterialMarker>;
pub type BlockMaterialId = MaterialId<BlockMaterialMarker>;

// === Context === //

type VoxelDataWriteCx<'a> = Cx<&'a mut ChunkVoxelData>;
type VoxelDataReadCx<'a> = Cx<&'a ChunkVoxelData>;

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

	#[clippy::dangerous(
		direct_chunk_loading,
		reason = "chunk loading should be handled by the dedicated chunk loading system"
	)]
	pub fn insert_chunk(
		&mut self,
		cx: VoxelDataWriteCx<'_>,
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
			//
			// Noalias: `internal_unlink` only borrows components in the world while `old` is now out
			// of the world.
			self.internal_unlink(cx!(noalias cx), &mut old.get_mut_s(cx!(noalias cx)));
		}

		// Set the chunk's main state
		let mut chunk_state = chunk.get_mut_s(cx!(cx));
		chunk_state.pos = pos;

		// Link the chunk to its neighbors
		for (face, neighbor) in chunk_state.neighbors.iter_mut() {
			let neighbor_chunk = self.get_chunk(pos + face.unit_typed::<ChunkVec>());
			*neighbor = neighbor_chunk;

			if let Some(neighbor_chunk) = neighbor_chunk {
				// Noalias: we know that neighboring chunks will never be the same chunk as our main
				// chunk.
				neighbor_chunk.get_mut_s(cx!(noalias cx)).neighbors[face.invert()] = Some(chunk);
			}
		}

		// Add the chunk to the dirty queue
		chunk_state.dirty_index = self.dirty.len();
		self.dirty.push(chunk);

		old
	}

	#[clippy::dangerous(
		direct_chunk_loading,
		reason = "chunk loading should be handled by the dedicated chunk loading system"
	)]
	pub fn remove_chunk(
		&mut self,
		cx: VoxelDataWriteCx<'_>,
		pos: ChunkVec,
	) -> Option<OwnedObj<ChunkVoxelData>> {
		let chunk = self.pos_map.remove(&pos);

		if let Some(chunk) = &chunk {
			// Noalias: `internal_unlink` only borrows components in the world while `old` is now
			// out of the world.
			self.internal_unlink(cx!(noalias cx), &mut chunk.get_mut_s(cx!(noalias cx)));
		}

		chunk
	}

	pub fn get_chunk(&self, pos: ChunkVec) -> Option<Obj<ChunkVoxelData>> {
		self.pos_map.get(&pos).map(OwnedObj::obj)
	}

	pub fn read_chunk<'a>(
		&'a self,
		cx: VoxelDataReadCx<'a>,
		chunk: Obj<ChunkVoxelData>,
	) -> CompRef<'a, ChunkVoxelData> {
		chunk.get_s(cx)
	}

	pub fn write_chunk<'a>(
		&'a mut self,
		cx: VoxelDataWriteCx<'a>,
		chunk: Obj<ChunkVoxelData>,
	) -> ChunkVoxelDataMut<'a> {
		ChunkVoxelDataMut {
			world: self,
			chunk,
			chunk_state: chunk.get_mut_s(cx),
		}
	}

	#[clippy::dangerous(
		direct_voxel_data_flush,
		reason = "the world should only be flushed by its dedicated chunk update system"
	)]
	pub fn flush_dirty(&mut self, cx: VoxelDataWriteCx<'_>) -> Vec<Obj<ChunkVoxelData>> {
		let dirty = mem::take(&mut self.dirty);

		for &dirty in &dirty {
			dirty.get_mut_s(cx!(cx)).dirty_index = usize::MAX;
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

	fn internal_unlink(&mut self, cx: VoxelDataWriteCx<'_>, chunk_state: &mut ChunkVoxelData) {
		// Unlink from neighbors
		for (face, neighbor) in chunk_state.neighbors.iter() {
			if let Some(neighbor) = neighbor {
				neighbor.get_mut_s(cx!(cx)).neighbors[face.invert()] = None;
			}
		}

		// Remove from dirty queue
		if chunk_state.dirty_index != usize::MAX {
			self.dirty.swap_remove(chunk_state.dirty_index);

			if let Some(displaced) = self.dirty.get(chunk_state.dirty_index) {
				displaced.get_mut_s(cx!(cx)).dirty_index = chunk_state.dirty_index;
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
	pub fn with_default_air_data(mut self) -> Self {
		self.blocks = Some(default_block_vector());
		self
	}

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
	chunk_state: CompMut<'a, ChunkVoxelData>,
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
		self.chunk_state.blocks = Some(data.unwrap_or_else(default_block_vector));
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

fn default_block_vector() -> Box<[Block; CHUNK_VOLUME as usize]> {
	boxed_arr_from_fn(|| Block::AIR)
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
	pub material: BlockMaterialId,
	pub variant: u8,
	pub light: u8,
}

impl Block {
	pub const AIR: Self = Self::new(MaterialId::AIR);

	pub const fn new(material: BlockMaterialId) -> Self {
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

	pub fn set_cached_chunk(&mut self, cache: Option<Obj<ChunkVoxelData>>) {
		self.cache = cache;
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

	pub fn set_pos(&mut self, world: Option<(VoxelDataReadCx<'_>, &WorldVoxelData)>, pos: V) {
		// Update `pos` and determine chunk delta
		let old_chunk = self.voxel_pos().chunk();
		self.pos = pos;
		let new_chunk = self.voxel_pos().chunk();

		// If the chunk changed and we have a cache, update it.
		if let Some(cache) = self.cache.filter(|_| old_chunk != new_chunk) {
			if let (Some((cx, world)), Some(face)) = (
				world,
				BlockFace::from_vec((new_chunk - old_chunk).to_glam()),
			) {
				debug_assert_eq!(self.cache, world.get_chunk(old_chunk));
				self.cache = world.read_chunk(cx!(cx), cache).neighbor(face);
				debug_assert_eq!(self.cache, world.get_chunk(new_chunk));
			} else {
				self.cache = None;
			}
		}
	}

	pub fn move_by(&mut self, world: Option<(VoxelDataReadCx<'_>, &WorldVoxelData)>, rel: V) {
		self.set_pos(world, self.pos + rel);
	}

	pub fn move_to_neighbor(
		&mut self,
		world: Option<(VoxelDataReadCx<'_>, &WorldVoxelData)>,
		face: BlockFace,
	) {
		self.move_by(world, face.unit_typed());
	}

	// === Operations === //

	pub fn at_absolute(
		&self,
		world: Option<(VoxelDataReadCx<'_>, &WorldVoxelData)>,
		pos: V,
	) -> Self {
		let mut clone = *self;
		clone.set_pos(world, pos);
		clone
	}

	pub fn at_relative(
		&self,
		world: Option<(VoxelDataReadCx<'_>, &WorldVoxelData)>,
		rel: V,
	) -> Self {
		let mut clone = *self;
		clone.move_by(world, rel);
		clone
	}

	pub fn at_neighbor(
		&self,
		world: Option<(VoxelDataReadCx<'_>, &WorldVoxelData)>,
		face: BlockFace,
	) -> Self {
		let mut clone = *self;
		clone.move_to_neighbor(world, face);
		clone
	}

	// === Aliases === //

	pub fn state(&mut self, cx: VoxelDataReadCx<'_>, world: &WorldVoxelData) -> Option<Block> {
		let chunk = self.chunk(world)?;
		world
			.read_chunk(cx!(cx), chunk)
			.block(self.voxel_pos().block())
	}

	pub fn try_set_state(
		&mut self,
		cx: VoxelDataWriteCx<'_>,
		world: &mut WorldVoxelData,
		block: Block,
	) -> Result<(), TrySetStateError> {
		if let Some(chunk) = self.chunk(world) {
			world
				.write_chunk(cx!(cx), chunk)
				.try_set_block(self.voxel_pos().block(), block)
				.then_some(())
				.ok_or(TrySetStateError::ChunkNotLoaded)
		} else {
			Err(TrySetStateError::OutOfWorld)
		}
	}

	pub fn set_state_or_warn(
		&mut self,
		cx: VoxelDataWriteCx<'_>,
		world: &mut WorldVoxelData,
		block: Block,
	) {
		if let Err(err) = self.try_set_state(cx!(cx), world, block) {
			log::warn!(
				"Attempted to write block outside of world or in an unloaded chunk. \
				 Specific error: {:?}, Requested position: {:?}, Material: {:?}.",
				err,
				self.pos,
				block,
			);
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

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum TrySetStateError {
	OutOfWorld,
	ChunkNotLoaded,
}
