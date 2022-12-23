use crucible_core::{
	ecs::{
		entity::Entity,
		provider::{DynProvider, Provider},
		storage::{CelledStorage, CelledStorageView},
	},
	lang::{
		iter::{ContextualIter, VolumetricIter, WithContext},
		polyfill::OptionPoly,
	},
	mem::c_enum::CEnum,
};
use smallvec::SmallVec;
use typed_glam::{
	ext::VecExt,
	traits::{SignedNumericVector3, VecFrom},
};

use crate::voxel::math::{Axis3, BlockFace, EntityVecExt, Line3, Sign, Vec3Ext, WorldVecExt};

use super::{
	data::{BlockState, VoxelChunkData, VoxelWorldData},
	math::{ChunkVec, EntityVec, WorldVec},
};

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
	WorldVec: VecFrom<V>,
	V: VecFrom<WorldVec>,
	V: SignedNumericVector3,
{
	pub fn new(world: &VoxelWorldData, pos: V) -> Self {
		Self {
			pos,
			chunk_cache: world.get_chunk(WorldVec::cast_from(pos).chunk()),
		}
	}

	pub fn new_uncached(pos: V) -> Self {
		Self {
			pos,
			chunk_cache: None,
		}
	}

	pub fn refresh(&mut self, (world,): (&VoxelWorldData,)) {
		self.chunk_cache = world.get_chunk(WorldVec::cast_from(self.pos).chunk());
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

	pub fn chunk(&mut self, (world,): (&VoxelWorldData,)) -> Option<Entity> {
		match self.chunk_cache {
			Some(chunk) => Some(chunk),
			None => {
				self.refresh((world,));
				self.chunk_cache
			}
		}
	}

	pub fn move_to_neighbor(
		&mut self,
		(world, chunks): (&VoxelWorldData, &CelledStorageView<VoxelChunkData>),
		face: BlockFace,
	) {
		// Update position
		let old_pos = self.pos;
		self.pos += face.unit_typed::<V>();

		// Update chunk cache
		if WorldVec::cast_from(old_pos).chunk() != WorldVec::cast_from(self.pos).chunk() {
			if let Some(chunk) = self.chunk_cache {
				self.chunk_cache = chunks.borrow(chunk).neighbor(face);
			} else {
				self.refresh((world,));
			}
		}
	}

	pub fn at_neighbor(
		mut self,
		cx: (&VoxelWorldData, &CelledStorageView<VoxelChunkData>),
		face: BlockFace,
	) -> Self {
		self.move_to_neighbor(cx, face);
		self
	}

	pub fn move_to(
		&mut self,
		(world, chunks): (&VoxelWorldData, &CelledStorageView<VoxelChunkData>),
		new_pos: V,
	) {
		let chunk_delta =
			WorldVec::cast_from(new_pos).chunk() - WorldVec::cast_from(self.pos).chunk();

		if let (Some(chunk), Some(face)) =
			(self.chunk_cache, BlockFace::from_vec(chunk_delta.to_glam()))
		{
			self.pos = new_pos;
			self.chunk_cache = chunks.borrow(chunk).neighbor(face);
		} else {
			self.pos = new_pos;
			self.refresh((world,));
		}
	}

	pub fn at_absolute(
		mut self,
		cx: (&VoxelWorldData, &CelledStorageView<VoxelChunkData>),
		new_pos: V,
	) -> Self {
		self.move_to(cx, new_pos);
		self
	}

	pub fn move_relative(
		&mut self,
		cx: (&VoxelWorldData, &CelledStorageView<VoxelChunkData>),
		delta: V,
	) {
		self.move_to(cx, self.pos + delta);
	}

	pub fn at_relative(
		mut self,
		cx: (&VoxelWorldData, &CelledStorageView<VoxelChunkData>),
		delta: V,
	) -> Self {
		self.move_relative(cx, delta);
		self
	}

	pub fn state(
		&mut self,
		(world, chunks): (&VoxelWorldData, &CelledStorageView<VoxelChunkData>),
	) -> Option<BlockState> {
		self.chunk((world,)).map(|chunk| {
			chunks
				.borrow(chunk)
				.block_state(WorldVec::cast_from(self.pos).block())
		})
	}

	pub fn set_state(
		&mut self,
		(world, chunks): (&mut VoxelWorldData, &CelledStorageView<VoxelChunkData>),
		state: BlockState,
	) {
		let chunk = match self.chunk((world,)) {
			Some(chunk) => chunk,
			None => {
				log::warn!("`set_state` called on `BlockLocation` outside of the world.");
				return;
			}
		};

		chunks.borrow_mut(chunk).set_block_state(
			(&chunk, world),
			WorldVec::cast_from(self.pos).block(),
			state,
		);
	}

	pub fn set_state_or_create(
		&mut self,
		(world, chunks, mut extra): (
			&mut VoxelWorldData,
			&mut CelledStorage<VoxelChunkData>,
			impl Provider,
		),
		factory: impl FnOnce(&mut DynProvider, ChunkVec) -> Entity,
		state: BlockState,
	) {
		// Fetch chunk
		let chunk = match self.chunk((world,)) {
			Some(chunk) => chunk,
			None => {
				let pos = WorldVec::cast_from(self.pos).chunk();
				let chunk = factory(&mut extra.as_dyn(), pos);
				world.add_chunk((chunks,), pos, chunk);
				chunk
			}
		};

		// Set block state
		chunks.get_mut(chunk).set_block_state(
			(&chunk, world),
			WorldVec::cast_from(self.pos).block(),
			state,
		);
	}

	pub fn as_block_location(&self) -> BlockLocation {
		BlockLocation {
			chunk_cache: self.chunk_cache,
			pos: WorldVec::cast_from(self.pos),
		}
	}

	pub fn iter_volume<'a>(
		self,
		cx: (&'a VoxelWorldData, &'a CelledStorageView<VoxelChunkData>),
		size: WorldVec,
	) -> impl Iterator<Item = Self> + 'a {
		debug_assert!(size.all(|v| u32::try_from(v).is_ok()));

		// TODO: Use the more efficient implementation below
		// 		let mut fingers = [self; 3];
		// 		let mut iter = VolumetricIter::new([size.x() as u32, size.y() as u32, size.z() as u32]);
		//
		// 		std::iter::from_fn(move || {
		// 			let [x, y, z] = iter.next_capturing(|i| {
		// 				if i > 0 {
		// 					fingers[i] = fingers[i - 1];
		// 					fingers[i - 1].move_to_neighbor(
		// 						cx,
		// 						match i {
		// 							1 => BlockFace::PositiveX,
		// 							2 => BlockFace::PositiveY,
		// 							_ => unreachable!(),
		// 						},
		// 					)
		// 				}
		// 			})?;
		//
		// 			let curr = fingers[2];
		//
		// 			fn workaround(a: WorldVec, b: WorldVec) -> WorldVec {
		// 				a + b
		// 			}
		//
		// 			debug_assert_eq!(
		// 				workaround(
		// 					WorldVec::cast_from(self.pos()),
		// 					WorldVec::new(x as i32, y as i32, z as i32)
		// 				),
		// 				WorldVec::cast_from(curr.pos())
		// 			);
		//
		// 			fingers[2].move_to_neighbor(cx, BlockFace::PositiveZ);
		//
		// 			Some(curr)
		// 		})

		VolumetricIter::new([size.x() as u32, size.y() as u32, size.z() as u32]).map(
			move |[x, y, z]| {
				self.at_relative(cx, WorldVec::new(x as i32, y as i32, z as i32).cast())
			},
		)
	}
}

// === RayCast === //

#[derive(Debug, Clone)]
pub struct RayCast {
	loc: EntityLocation,
	dir: EntityVec,
	dist: f64,
}

impl RayCast {
	pub fn new_at(loc: EntityLocation, dir: EntityVec) -> Self {
		let (dir, dist) = {
			let dir_len_recip = dir.length_recip();

			if dir_len_recip.is_finite() && dir_len_recip > 0.0 {
				(dir * dir_len_recip, 1.)
			} else {
				(EntityVec::ZERO, f64::INFINITY)
			}
		};

		Self { loc, dir, dist }
	}

	pub fn new_uncached(pos: EntityVec, dir: EntityVec) -> Self {
		Self::new_at(EntityLocation::new_uncached(pos), dir)
	}

	pub fn loc(&mut self) -> &mut EntityLocation {
		&mut self.loc
	}

	pub fn pos(&self) -> EntityVec {
		self.loc.pos()
	}

	pub fn dir(&self) -> EntityVec {
		self.dir
	}

	pub fn dist(&self) -> f64 {
		self.dist
	}

	pub fn step(
		&mut self,
		cx: (&VoxelWorldData, &CelledStorageView<VoxelChunkData>),
	) -> SmallVec<[RayCastIntersection; 3]> {
		let mut intersections = SmallVec::<[RayCastIntersection; 3]>::new();

		// Collect intersections
		let mut block_loc = self.loc.as_block_location();
		{
			let step_line = Line3::new_origin_delta(self.pos(), self.dir);
			self.loc.move_relative(cx, self.dir);

			let start_block = step_line.start.block_pos();
			let end_block = step_line.end.block_pos();
			let block_delta = end_block - start_block;

			for axis in Axis3::variants() {
				let delta = block_delta.comp(axis);
				debug_assert!((-1..=1).contains(&delta));

				let sign = match Sign::of(delta) {
					Some(sign) => sign,
					None => continue,
				};

				let face = BlockFace::compose(axis, sign);

				let isect_layer = start_block.block_interface_layer(face);
				let (isect_lerp, isect_pos) = axis.plane_intersect(isect_layer, step_line);

				intersections.push(RayCastIntersection {
					_non_exhaustive: (),
					block: block_loc, // This will be updated in a bit.
					face,
					distance: self.dist + isect_lerp,
					pos: isect_pos,
				});
			}

			intersections.sort_by(|a, b| a.distance.total_cmp(&b.distance));
		}

		// Update block positions
		for isect in &mut intersections {
			isect.block = block_loc.at_neighbor(cx, isect.face);
			block_loc = isect.block;
		}

		// Update distance accumulator
		// N.B. the direction is either normalized, in which case the step was of length 1, or we're
		// traveling with direction zero, in which case the distance is already infinite.
		self.dist += 1.;

		intersections
	}

	pub fn step_for<'a>(
		&'a mut self,
		cx: (&'a VoxelWorldData, &'a CelledStorageView<VoxelChunkData>),
		max_dist: f64,
	) -> RayCastIter<'a> {
		ContextualRayCastIter::new(max_dist).with_context((cx, self))
	}
}

#[derive(Debug, Clone)]
pub struct RayCastIntersection {
	_non_exhaustive: (),
	pub block: BlockLocation,
	pub face: BlockFace,
	pub pos: EntityVec,
	pub distance: f64,
}

pub type RayCastIter<'a> = WithContext<
	(
		(&'a VoxelWorldData, &'a CelledStorageView<VoxelChunkData>),
		&'a mut RayCast,
	),
	ContextualRayCastIter,
>;

#[derive(Debug, Clone)]
pub struct ContextualRayCastIter {
	pub max_dist: f64,
	back_log: SmallVec<[RayCastIntersection; 3]>,
}

impl ContextualRayCastIter {
	pub fn new(max_dist: f64) -> Self {
		Self {
			max_dist,
			back_log: Default::default(),
		}
	}
}

impl<'a>
	ContextualIter<(
		(&'a VoxelWorldData, &'a CelledStorageView<VoxelChunkData>),
		&'a mut RayCast,
	)> for ContextualRayCastIter
{
	type Item = RayCastIntersection;

	fn next_on_ref(
		&mut self,
		(cx, ray): &mut (
			(&'a VoxelWorldData, &'a CelledStorageView<VoxelChunkData>),
			&'a mut RayCast,
		),
	) -> Option<Self::Item> {
		let cx = *cx;

		while self.back_log.is_empty() {
			self.back_log = ray.step(cx);
		}

		let next = if !self.back_log.is_empty() {
			self.back_log.remove(0)
		} else if ray.dist() < self.max_dist {
			self.back_log = ray.step(cx);

			// It is possible that the ray needs to travel more than 1 unit to get out of a block.
			// The furthest it can travel is `sqrt(3 * 1^2) ~= 1.7` so we have to call this method
			// at most twice. Also, yes, this handles a zero-vector direction because rays with no
			// direction automatically get infinite distance.
			if self.back_log.is_empty() {
				self.back_log = ray.step(cx);
			}

			self.back_log.remove(0)
		} else {
			return None;
		};

		if next.distance <= self.max_dist {
			Some(next)
		} else {
			None
		}
	}
}

// === Collisions === //

pub fn move_rigid_body(
	cx: (&VoxelWorldData, &CelledStorageView<VoxelChunkData>),
	src: EntityVec,
	size: EntityVec,
	delta: EntityVec,
) -> EntityVec {
	let mut loc = EntityLocation::new(&cx.0, src);

	'a: for movement_axis in Axis3::variants() {
		// Decompose delta
		let movement_unit = movement_axis.unit_typed::<EntityVec>();
		let movement_delta = delta.comp(movement_axis);
		let movement_magnitude = movement_delta.abs();
		let movement_sign = Sign::of(movement_delta).unwrap_or(Sign::Positive);

		// Determine the occlusion candidate volume
		let volume_start = match movement_sign {
			// Align to the far side of the collider
			Sign::Positive => loc.at_relative(cx, movement_unit * size.comp(movement_axis)),
			// Align to the destination corner
			Sign::Negative => loc.at_relative(cx, -movement_unit * movement_magnitude),
		};
		let mut volume_size = size;
		*volume_size.comp_mut(movement_axis) = movement_magnitude;

		let volume_end = volume_start.pos() + volume_size;
		let volume_size = volume_end.block_pos() - volume_start.pos().block_pos();

		// Check volume for occluding blocks
		for mut occluder in volume_start.iter_volume(cx, volume_size) {
			if occluder
				.state(cx)
				.p_is_some_and(|state| state.material != 0)
			{
				// TODO: Allow partial movements
				continue 'a;
			}
		}

		// Otherwise, commit the movement in full
		loc.move_relative(cx, movement_unit * movement_delta);
	}

	loc.pos()
}