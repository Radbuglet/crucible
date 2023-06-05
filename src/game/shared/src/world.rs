use std::cell::RefMut;

use bort::{Obj, OwnedObj};
use crucible_foundation_shared::{
	actor::spatial::SpatialTracker,
	math::WorldVec,
	voxel::data::{Block, BlockVoxelPointer, WorldVoxelData},
};
use crucible_util::mem::hash::FxHashMap;

// === State === //

#[derive(Debug, Default)]
pub struct WorldManager {
	worlds: FxHashMap<String, OwnedObj<WorldManagedData>>,
}

impl WorldManager {
	pub fn create_world(
		&mut self,
		id: impl ToString,
		world: OwnedObj<WorldManagedData>,
	) -> Obj<WorldManagedData> {
		let (world, world_ref) = world.split_guard();
		self.worlds.insert(id.to_string(), world);
		world_ref
	}

	pub fn mutate_world(&mut self, world: Obj<WorldManagedData>) -> WorldViewMut<'_> {
		WorldViewMut::new(world)
	}
}

#[derive(Debug, Default)]
pub struct WorldManagedData {
	_private: (),
}

// === Views === //

#[derive(Debug)]
#[non_exhaustive]
pub struct WorldViewMut<'a> {
	pub managed: RefMut<'a, WorldManagedData>,
	pub voxel_data: RefMut<'a, WorldVoxelData>,
	pub spatials: RefMut<'a, SpatialTracker>,
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

	#[must_use]
	pub fn try_set_block(&mut self, pos: WorldVec, block: Block) -> bool {
		self.location_cache.set_pos(Some(&self.voxel_data), pos);
		self.location_cache
			.try_set_state(&mut self.voxel_data, block)
	}
}
