use std::{collections::hash_map, mem};

use bevy_autoken::{
    random_component, random_event, send_event, spawn_entity, Obj, RandomAccess, RandomEntityExt,
};
use bevy_ecs::{event::Event, removal_detection::RemovedComponents, system::Query};
use crucible_math::{
    Axis3, BlockFace, BlockVec, BlockVecExt, ChunkVec, EntityVec, Sign, VecCompExt, WorldVec,
    WorldVecExt, CHUNK_VOLUME,
};
use crucible_utils::newtypes::{define_index, EnumIndex as _, IndexArray};
use rustc_hash::{FxHashMap, FxHashSet};
use typed_glam::traits::{CastVecFrom, NumericVector};

use crate::material::{MaterialCache, MaterialRegistry};

// === Block Structures === //

// Materials
pub type BlockMaterialRegistry = MaterialRegistry<BlockMaterial>;

random_component!(BlockMaterialRegistry);

pub type BlockMaterialCache<V> = MaterialCache<BlockMaterial, V>;

define_index! {
    pub struct BlockMaterial: u16;
}

impl BlockMaterial {
    pub const AIR: Self = Self(0);

    pub fn is_air(self) -> bool {
        self == Self::AIR
    }

    pub fn is_not_air(self) -> bool {
        self != Self::AIR
    }
}

// Block Data
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct BlockData {
    pub material: BlockMaterial,
    pub variant: u32,
}

impl BlockData {
    pub const AIR: Self = Self {
        material: BlockMaterial::AIR,
        variant: 0,
    };

    pub fn new(material: BlockMaterial) -> Self {
        Self {
            material,
            variant: 0,
        }
    }

    pub fn is_air(&self) -> bool {
        self.material.is_air()
    }

    pub fn is_not_air(&self) -> bool {
        self.material.is_not_air()
    }
}

// === Events === //

#[derive(Debug, Copy, Clone, Event)]
pub struct WorldChunkCreated {
    pub world: Obj<WorldVoxelData>,
    pub chunk: Obj<ChunkVoxelData>,
}

random_event!(WorldChunkCreated);

// === Components === //

#[derive(Debug, Default)]
pub struct WorldVoxelData {
    chunks: FxHashMap<ChunkVec, Obj<ChunkVoxelData>>,
    dirty: FxHashSet<Obj<ChunkVoxelData>>,
}

random_component!(WorldVoxelData);

impl WorldVoxelData {
    pub fn get_or_insert(self: Obj<Self>, pos: ChunkVec) -> Obj<ChunkVoxelData> {
        let world = self.deref_mut();
        let entry = match world.chunks.entry(pos) {
            hash_map::Entry::Occupied(entry) => return *entry.into_mut(),
            hash_map::Entry::Vacant(entry) => entry,
        };

        let mut chunk = spawn_entity(()).insert(ChunkVoxelData {
            world: self,
            pos,
            neighbors: IndexArray::default(),
            data: None,
            non_air_count: 0,
            is_dirty: false,
        });

        entry.insert(chunk);

        for face in BlockFace::variants() {
            let neighbor = pos + face.unit();

            let Some(&(mut neighbor)) = world.chunks.get(&neighbor) else {
                continue;
            };

            neighbor.neighbors[face.invert()] = Some(chunk);
            chunk.neighbors[face] = Some(neighbor);
        }

        send_event(WorldChunkCreated { world: self, chunk });

        chunk
    }

    pub fn get(&self, pos: ChunkVec) -> Option<Obj<ChunkVoxelData>> {
        self.chunks.get(&pos).copied()
    }

    pub fn iter_dirty(&self) -> impl Iterator<Item = Obj<ChunkVoxelData>> + '_ {
        self.dirty.iter().copied()
    }

    pub fn clear_dirty(&mut self) {
        for mut chunk in self.dirty.drain() {
            chunk.is_dirty = false;
        }
    }
}

#[derive(Debug)]
pub struct ChunkVoxelData {
    world: Obj<WorldVoxelData>,
    pos: ChunkVec,
    neighbors: IndexArray<BlockFace, Option<Obj<ChunkVoxelData>>>,
    data: Option<ChunkData>,
    non_air_count: i32,
    is_dirty: bool,
}

#[derive(Debug, Clone)]
pub enum ChunkData {
    AllAir,
    Complex(Box<[BlockData; CHUNK_VOLUME as usize]>),
}

random_component!(ChunkVoxelData);

impl ChunkVoxelData {
    pub fn world(&self) -> Obj<WorldVoxelData> {
        self.world
    }

    pub fn pos(&self) -> ChunkVec {
        self.pos
    }

    pub fn neighbor(&self, face: BlockFace) -> Option<Obj<ChunkVoxelData>> {
        self.neighbors[face]
    }

    pub fn initialize_data(&mut self, data: ChunkData) {
        debug_assert!(self.data.is_none());

        self.data = Some(data);
    }

    pub fn is_init(&self) -> bool {
        self.data.is_some()
    }

    pub fn block(&self, block: BlockVec) -> Option<BlockData> {
        self.data.as_ref().map(|v| match v {
            ChunkData::AllAir => BlockData::AIR,
            ChunkData::Complex(v) => v[block.to_index()],
        })
    }

    pub fn block_or_air(&self, block: BlockVec) -> BlockData {
        self.block(block).unwrap_or(BlockData::AIR)
    }

    pub fn set_block(mut self: Obj<Self>, block: BlockVec, new_data: BlockData) {
        self.set_block_no_dirty(block, new_data);
        self.mark_dirty();
    }

    pub fn set_block_no_dirty(&mut self, block: BlockVec, new_data: BlockData) {
        // Ensure that the chunk is loaded.
        let Some(data) = &mut self.data else {
            tracing::warn!("attempted to set block state in unloaded chunk");
            return;
        };

        // Promote all-air chunks into complex chunks
        let data = match data {
            ChunkData::AllAir => {
                *data = ChunkData::Complex(Box::new([BlockData::AIR; CHUNK_VOLUME as usize]));

                match data {
                    ChunkData::AllAir => unreachable!(),
                    ChunkData::Complex(data) => data,
                }
            }
            ChunkData::Complex(data) => data,
        };

        // Update the block state
        let old_data = mem::replace(&mut data[block.to_index()], new_data);

        let was_air = old_data.material.is_air() as i8;
        let is_air = new_data.material.is_air() as i8;
        self.non_air_count += (is_air - was_air) as i32;
    }

    pub fn mark_dirty(mut self: Obj<Self>) {
        if self.is_dirty {
            return;
        }

        self.deref_mut().world.dirty.insert(self);
        self.is_dirty = true;
    }

    pub fn non_air_count(&self) -> i32 {
        self.non_air_count
    }

    pub fn attempt_simplification(&mut self) {
        if self.non_air_count == 0 && self.data.is_some() {
            self.data = Some(ChunkData::AllAir);
        }
    }

    pub fn unlink(self: Obj<Self>) {
        let mut world = self.world;
        world.chunks.remove(&self.pos);

        for face in BlockFace::variants() {
            let neighbor = self.neighbors[face];

            let Some(mut neighbor) = neighbor else {
                continue;
            };

            neighbor.neighbors[face.invert()] = None;
        }
    }
}

// === Pointer === //

pub type WorldPointer = VoxelPointer<WorldVec>;
pub type EntityPointer = VoxelPointer<EntityVec>;

#[derive(Debug, Copy, Clone, Default)]
pub struct VoxelPointer<V> {
    pub chunk: Option<Obj<ChunkVoxelData>>,
    pub pos: V,
}

impl<V> VoxelPointer<V>
where
    WorldVec: CastVecFrom<V>,
    V: NumericVector,
{
    pub const fn new(vector: V) -> Self {
        Self {
            chunk: None,
            pos: vector,
        }
    }

    pub fn move_to(&mut self, pos: V) -> &mut Self {
        // Update vector
        let old_chunk = self.block().chunk();
        self.pos = pos;
        let new_chunk = self.block().chunk();

        // Update chunk
        if old_chunk == new_chunk {
            // (we're still in the same chunk)
            return self;
        }

        let Some(mut chunk) = self.chunk else {
            // (the chunk cache is unset and we don't want to recompute it)
            return self;
        };

        self.chunk = None;

        let delta = new_chunk - old_chunk;

        for axis in Axis3::variants() {
            let delta = delta.comp(axis);

            // We can't move more than one chunk across
            if !(-1..=1).contains(&delta) {
                return self;
            }

            // If the delta is zero, don't move anywhere.
            let Some(sign) = Sign::of(delta) else {
                continue;
            };

            // If the neighbor is none, our cache becomes none.
            let Some(neighbor) = chunk.neighbor(BlockFace::compose(axis, sign)) else {
                return self;
            };

            chunk = neighbor;
        }

        self.chunk = Some(chunk);
        self
    }

    pub fn move_by(&mut self, delta: V) -> &mut Self {
        self.move_to(self.pos + delta)
    }

    #[must_use]
    pub fn moved_to(mut self, pos: V) -> Self {
        self.move_to(pos);
        self
    }

    #[must_use]
    pub fn moved_by(mut self, delta: V) -> Self {
        self.move_by(delta);
        self
    }

    pub fn block(&self) -> WorldVec {
        self.pos.cast::<WorldVec>()
    }

    pub fn block_pointer(self) -> WorldPointer {
        WorldPointer {
            chunk: self.chunk,
            pos: self.block(),
        }
    }

    pub fn chunk(&mut self, world: Obj<WorldVoxelData>) -> Option<Obj<ChunkVoxelData>> {
        match self.chunk {
            Some(chunk) => Some(chunk),
            None => {
                self.chunk = world.get(self.block().decompose().0);
                self.chunk
            }
        }
    }

    pub fn chunk_or_insert(&mut self, world: Obj<WorldVoxelData>) -> Obj<ChunkVoxelData> {
        match self.chunk {
            Some(chunk) => chunk,
            None => {
                let chunk = world.get_or_insert(self.block().decompose().0);
                self.chunk = Some(chunk);
                chunk
            }
        }
    }
}

impl WorldPointer {
    pub fn move_to_neighbor(&mut self, face: BlockFace) -> &mut Self {
        // Update vector
        let old_pos = self.pos;
        self.pos += face.unit();
        let new_pos = self.pos;

        // Update chunk
        if old_pos.chunk() == new_pos.chunk() {
            // (we didn't move chunks)
            return self;
        }

        if let Some(chunk) = self.chunk {
            self.chunk = chunk.neighbor(face);
        }

        self
    }

    #[must_use]
    pub fn neighbor(mut self, face: BlockFace) -> Self {
        self.move_to_neighbor(face);
        self
    }

    pub fn state(&mut self, world: Obj<WorldVoxelData>) -> Option<BlockData> {
        self.chunk(world).and_then(|v| v.block(self.pos.block()))
    }

    pub fn state_or_air(&mut self, world: Obj<WorldVoxelData>) -> BlockData {
        self.chunk(world)
            .map_or(BlockData::AIR, |v| v.block_or_air(self.pos.block()))
    }

    pub fn set_state(
        &mut self,
        world: Obj<WorldVoxelData>,
        data: BlockData,
        mut policy: impl SetStatePolicy,
    ) {
        let chunk = match self.chunk {
            Some(chunk) => chunk,
            None => {
                let Some(chunk) = policy.fetch_chunk(world, self.pos.chunk()) else {
                    return;
                };
                self.chunk = Some(chunk);
                chunk
            }
        };

        policy.set_block(chunk, self.pos.block(), data);
    }
}

pub trait SetStatePolicy: Sized {
    fn fetch_chunk(
        &mut self,
        world: Obj<WorldVoxelData>,
        pos: ChunkVec,
    ) -> Option<Obj<ChunkVoxelData>>;

    fn set_block(&mut self, chunk: Obj<ChunkVoxelData>, pos: BlockVec, data: BlockData);
}

impl<T: SetStatePolicy> SetStatePolicy for &mut T {
    fn fetch_chunk(
        &mut self,
        world: Obj<WorldVoxelData>,
        pos: ChunkVec,
    ) -> Option<Obj<ChunkVoxelData>> {
        (*self).fetch_chunk(world, pos)
    }

    fn set_block(&mut self, chunk: Obj<ChunkVoxelData>, pos: BlockVec, data: BlockData) {
        (*self).set_block(chunk, pos, data)
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub struct KeepInWorld;

impl SetStatePolicy for KeepInWorld {
    fn fetch_chunk(
        &mut self,
        world: Obj<WorldVoxelData>,
        pos: ChunkVec,
    ) -> Option<Obj<ChunkVoxelData>> {
        world.get(pos)
    }

    fn set_block(&mut self, chunk: Obj<ChunkVoxelData>, pos: BlockVec, data: BlockData) {
        chunk.set_block(pos, data);
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub struct PopulateWorld;

impl SetStatePolicy for PopulateWorld {
    fn fetch_chunk(
        &mut self,
        world: Obj<WorldVoxelData>,
        pos: ChunkVec,
    ) -> Option<Obj<ChunkVoxelData>> {
        Some(world.get_or_insert(pos))
    }

    fn set_block(&mut self, mut chunk: Obj<ChunkVoxelData>, pos: BlockVec, data: BlockData) {
        if !chunk.is_init() {
            chunk.initialize_data(ChunkData::AllAir);
        }
        chunk.set_block(pos, data);
    }
}

// === Systems === //

pub fn sys_unlink_dead_chunks(
    mut rand: RandomAccess<(&mut WorldVoxelData, &mut ChunkVoxelData)>,
    mut query: RemovedComponents<Obj<ChunkVoxelData>>,
) {
    rand.provide(|| {
        for entity in query.read() {
            entity.get::<ChunkVoxelData>().unlink();
        }
    });
}

pub fn sys_clear_dirty_chunk_lists(
    mut rand: RandomAccess<(&mut WorldVoxelData, &mut ChunkVoxelData)>,
    mut query: Query<&Obj<WorldVoxelData>>,
) {
    rand.provide(|| {
        for &(mut world) in query.iter_mut() {
            world.clear_dirty();
        }
    });
}
