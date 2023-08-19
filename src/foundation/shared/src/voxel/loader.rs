use std::{
	fmt,
	ops::Range,
	time::{Duration, Instant},
};

use bort::{
	delegate,
	saddle::{cx, BortComponents},
	Obj, OwnedObj,
};

use crate::math::{Aabb3, ChunkVec, Sign};

use super::data::{ChunkVoxelData, VoxelDataWriteCx, WorldVoxelData};

// === Context === //

cx! {
	pub trait LoaderUpdateCx(BortComponents): VoxelDataWriteCx = mut LoadedChunk;
}

// === Region === //

pub trait Region: Sized + Clone + fmt::Debug {
	fn compare(add: Option<Self>, sub: Option<Self>, iter: impl FnMut(ChunkVec, Sign));
}

fn iter_added_dim(main: Range<i32>, exclude: Range<i32>) -> impl Iterator<Item = i32> {
	(main.start..exclude.start).chain(exclude.end..main.end)
}

fn iter_added_aabb(add: Aabb3<ChunkVec>, sub: Aabb3<ChunkVec>, mut iter: impl FnMut(ChunkVec)) {
	for x in iter_added_dim(
		add.origin.x()..add.max_corner().x(),
		sub.origin.x()..sub.max_corner().x(),
	) {
		for y in iter_added_dim(
			add.origin.y()..add.max_corner().y(),
			sub.origin.y()..sub.max_corner().y(),
		) {
			for z in iter_added_dim(
				add.origin.z()..add.max_corner().z(),
				sub.origin.z()..sub.max_corner().z(),
			) {
				iter(ChunkVec::new(x, y, z));
			}
		}
	}
}

impl Region for Aabb3<ChunkVec> {
	fn compare(add: Option<Self>, sub: Option<Self>, mut iter: impl FnMut(ChunkVec, Sign)) {
		let zero = Aabb3 {
			origin: ChunkVec::ZERO,
			size: ChunkVec::ZERO,
		};
		let add = add.unwrap_or(zero);
		let sub = sub.unwrap_or(zero);

		iter_added_aabb(add, sub, |pos| iter(pos, Sign::Positive));
		iter_added_aabb(sub, add, |pos| iter(pos, Sign::Negative));
	}
}

// === WorldLoader === //

#[derive(Debug)]
pub struct WorldLoader {
	factory: WorldChunkFactory,
	to_unload: Vec<Obj<LoadedChunk>>,
}

delegate! {
	pub fn WorldChunkFactory(world: &mut WorldVoxelData, pos: ChunkVec) -> OwnedObj<ChunkVoxelData>
}

impl WorldLoader {
	pub fn new(factory: WorldChunkFactory) -> Self {
		Self {
			factory,
			to_unload: Vec::new(),
		}
	}

	pub fn update_region<R: Region>(
		&mut self,
		cx: &impl LoaderUpdateCx,
		world: &mut WorldVoxelData,
		from_region: Option<R>,
		to_region: Option<R>,
		is_temporary: bool,
	) {
		let unload_at = Instant::now() + Duration::from_secs(15);

		R::compare(to_region, from_region, |pos, sign| {
			let chunk_obj = world
				.get_chunk(pos)
				.unwrap_or_else(|| {
					let (chunk, chunk_ref) = (self.factory)(world, pos).split_guard();
					world.insert_chunk(cx, pos, chunk);
					chunk_ref
				})
				.entity()
				.obj::<LoadedChunk>();

			let mut chunk = chunk_obj.get_mut_s(cx);

			// If the chunk was on the deletion queue but no longer is, remove it from the queue.
			if sign == Sign::Positive && chunk.rc == 0 && chunk.flag_loc != usize::MAX {
				self.to_unload.swap_remove(chunk.flag_loc);

				if let Some(moved) = self.to_unload.get(chunk.flag_loc) {
					moved.get_mut_s(cx).flag_loc = chunk.flag_loc;
				}

				chunk.flag_loc = usize::MAX;
			}

			// If the chunk was loaded but now no longer should be, add it to the unload queue.
			if (sign == Sign::Negative && chunk.rc == 1) ||
				// Alternatively, if this chunk is being temporarily loaded and no one else is keeping
				// it alive, queue its unloading.
				(is_temporary && chunk.rc == 0)
			{
				chunk.flag_loc = self.to_unload.len();
				self.to_unload.push(chunk_obj);
				chunk.unload_at = unload_at;
				chunk.rc = 0;
			}

			if !is_temporary {
				chunk.rc = chunk
					.rc
					.checked_add_signed(sign.unit_i64())
					.expect("too many references to the chunk");
			}
		});
	}

	pub fn load_region(
		&mut self,
		cx: &impl LoaderUpdateCx,
		world: &mut WorldVoxelData,
		new_region: impl Region,
	) {
		self.update_region(cx, world, None, Some(new_region), false);
	}

	pub fn unload_region(
		&mut self,
		cx: &impl LoaderUpdateCx,
		world: &mut WorldVoxelData,
		old_region: impl Region,
	) {
		self.update_region(cx, world, Some(old_region), None, false);
	}

	pub fn move_region<R: Region>(
		&mut self,
		cx: &impl LoaderUpdateCx,
		world: &mut WorldVoxelData,
		from_region: R,
		to_region: R,
	) {
		self.update_region(cx, world, Some(from_region), Some(to_region), false);
	}

	pub fn temp_load_region(
		&mut self,
		cx: &impl LoaderUpdateCx,
		world: &mut WorldVoxelData,
		new_region: impl Region,
	) {
		self.update_region(cx, world, None, Some(new_region), true);
	}
}

#[derive(Debug)]
pub struct LoadedChunk {
	rc: u64,
	flag_loc: usize,
	unload_at: Instant,
}

impl Default for LoadedChunk {
	fn default() -> Self {
		Self {
			rc: 0,
			flag_loc: usize::MAX,
			unload_at: Instant::now(),
		}
	}
}

impl LoadedChunk {
	pub fn rc(&self) -> u64 {
		self.rc
	}

	pub fn unload_at(&self) -> Instant {
		self.unload_at
	}
}