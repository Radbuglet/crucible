use std::cell::RefMut;

use bort::{Obj, OwnedObj};
use crucible_foundation_shared::{
	actor::spatial::SpatialManager,
	math::WorldVec,
	voxel::data::{Block, BlockVoxelPointer, WorldVoxelData},
};
use crucible_util::mem::hash::FxHashMap;

// === State === //

#[derive(Debug)]
pub struct WorldManager {
	worlds: FxHashMap<String, OwnedObj<WorldManagedData>>,
}

impl WorldManager {
	pub fn create_world(&mut self, id: impl ToString) {
		todo!()
	}

	pub fn unload_world(&mut self, id: &str) {
		todo!()
	}

	pub fn mutate_world(&mut self, world: Obj<WorldManagedData>) -> WorldViewMut<'_> {
		WorldViewMut::new(world)
	}

	pub fn get_world_mut(&mut self, id: &str) {
		todo!()
	}
}

#[derive(Debug)]
pub struct WorldManagedData {}

// === Views === //

#[derive(Debug)]
#[non_exhaustive]
pub struct WorldViewMut<'a> {
	pub managed: RefMut<'a, WorldManagedData>,
	pub voxel_data: RefMut<'a, WorldVoxelData>,
	pub spatials: RefMut<'a, SpatialManager>,
	pub location_cache: BlockVoxelPointer,
}

impl WorldViewMut<'_> {
	pub fn new(world: Obj<WorldManagedData>) -> Self {
		Self {
			managed: world.get_mut(),
			voxel_data: world.entity().get_mut(),
			spatials: world.entity().get_mut(),
			location_cache: BlockVoxelPointer::new_uncached(WorldVec::ZERO),
		}
	}

	pub fn try_get_block_uncached(&self, pos: WorldVec) -> Option<Block> {
		self.location_cache
			.at_absolute(Some(&self.voxel_data), pos)
			.state(&self.voxel_data)
	}

	pub fn try_get_block(&mut self, pos: WorldVec) -> Option<Block> {
		self.location_cache.set_pos(Some(&self.voxel_data), pos);
		self.location_cache.state(&self.voxel_data)
	}

	pub fn get_block_uncached(&self, pos: WorldVec) -> Block {
		self.try_get_block_uncached(pos).unwrap_or(Block::AIR)
	}

	pub fn get_block(&mut self, pos: WorldVec) -> Block {
		self.try_get_block(pos).unwrap_or(Block::AIR)
	}

	// TODO
}
