use std::{borrow::Cow, mem};

use hashbrown::HashMap;

use crucible_core::{
	debug::lifetime::Dependent,
	ecs::{
		entity::Entity,
		provider::{DynProvider, Provider},
		storage::{CelledStorage, CelledStorageView},
	},
	lang::iter::VolumetricIter,
	mem::{
		c_enum::{CEnum, CEnumMap},
		free_list::FreeList,
	},
};
use typed_glam::{
	ext::VecExt,
	traits::{SignedNumericVector3, VecFrom},
};

use super::math::{
	BlockFace, BlockVec, BlockVecExt, ChunkVec, EntityVec, WorldVec, WorldVecExt, CHUNK_VOLUME,
};

// === World === //

#[derive(Debug, Default)]
pub struct VoxelWorldData {
	pos_map: HashMap<ChunkVec, Entity>,
	flagged: Vec<Entity>,
}

impl VoxelWorldData {
	pub fn add_chunk(
		&mut self,
		(chunks,): (&mut CelledStorage<VoxelChunkData>,),
		pos: ChunkVec,
		chunk: Entity,
	) {
		debug_assert!(!self.pos_map.contains_key(&pos));

		// Create chunk
		chunks.add(
			chunk,
			VoxelChunkData {
				pos,
				flagged: None,
				neighbors: CEnumMap::default(),
				data: [0; CHUNK_VOLUME as usize],
			},
		);
		self.pos_map.insert(pos, chunk);

		// Link to neighbors
		let data = chunks.as_celled_view();
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

	pub fn remove_chunk(
		&mut self,
		(chunks,): (&mut CelledStorage<VoxelChunkData>,),
		pos: ChunkVec,
	) {
		let chunk = self.pos_map.remove(&pos).unwrap();
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

	pub fn flag_chunk(&mut self, (&chunk, chunk_data): (&Entity, &mut VoxelChunkData)) {
		if chunk_data.flagged.is_none() {
			chunk_data.flagged = Some(self.flagged.len());
			self.flagged.push(chunk);
		}
	}

	pub fn flush_flagged(
		&mut self,
		(chunks,): (&mut CelledStorage<VoxelChunkData>,),
	) -> Vec<Entity> {
		let flagged = mem::replace(&mut self.flagged, Vec::new());

		for &flagged in &flagged {
			chunks.get_mut(flagged).flagged = None;
		}

		flagged
	}
}

#[derive(Debug)]
pub struct VoxelChunkData {
	pos: ChunkVec,
	flagged: Option<usize>,
	neighbors: CEnumMap<BlockFace, Option<Entity>>,
	data: [u32; CHUNK_VOLUME as usize],
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
		(&me, world): (&Entity, &mut VoxelWorldData),
		pos: BlockVec,
		state: BlockState,
	) {
		let old = &mut self.data[pos.to_index()];
		let new = state.encode();

		if *old != new {
			*old = new;
			world.flag_chunk((&me, self));
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
		// 			// Workaround for #81448
		// 			// TODO: Remove when fixed
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

// === MaterialRegistry === //

#[derive(Debug, Default)]
pub struct MaterialRegistry {
	slots: FreeList<Dependent<Entity>, u16>,
	id_map: HashMap<Cow<'static, str>, u16>,
}

impl MaterialRegistry {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn register(&mut self, id: impl Into<Cow<'static, str>>, descriptor: Entity) -> u16 {
		let (_, slot) = self.slots.add(descriptor.into());

		let id = id.into();
		if let Err(e) = self.id_map.try_insert(id, slot) {
			log::error!("Registered duplicate material with id {:?}.", e.entry.key());
		}

		slot
	}

	pub fn try_resolve_id(&self, id: &str) -> Option<u16> {
		self.id_map.get(id).copied()
	}

	pub fn resolve_slot(&self, slot: u16) -> Entity {
		self.slots.get(slot).get()
	}

	pub fn unregister(&mut self, id: &str) {
		let Some(slot) = self.id_map.remove(id) else {
			log::error!("Attempted to unregister material under non-existent ID {:?}.", id);
			return;
		};

		self.slots.remove(slot);
	}
}
