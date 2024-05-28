use bevy_autoken::{random_component, Obj, RandomEntityExt};
use bevy_ecs::entity::Entity;
use crucible_math::{
    Axis3, BlockFace, ChunkVec, EntityVec, Sign, VecCompExt, WorldVec, WorldVecExt, CHUNK_VOLUME,
};
use newtypes::{NumEnum, NumEnumMap};
use rustc_hash::FxHashMap;
use typed_glam::traits::{CastVecFrom, NumericVector};

// === Structures === //

#[derive(Debug, Default)]
pub struct WorldVoxelData {
    chunks: FxHashMap<ChunkVec, Obj<ChunkVoxelData>>,
}

impl WorldVoxelData {
    pub fn insert(mut self: Obj<Self>, pos: ChunkVec, chunk: Entity) {
        debug_assert!(!chunk.has::<ChunkVoxelData>());
        debug_assert!(!self.chunks.contains_key(&pos));

        let mut chunk = chunk.insert(ChunkVoxelData {
            world: self,
            pos,
            neighbors: NumEnumMap::default(),
            data: None,
        });

        self.chunks.insert(pos, chunk);

        for face in BlockFace::variants() {
            let neighbor = pos + face.unit();

            let Some(&(mut neighbor)) = self.chunks.get(&neighbor) else {
                continue;
            };

            neighbor.neighbors[face.invert()] = Some(chunk);
            chunk.neighbors[face] = Some(neighbor);
        }
    }

    pub fn remove(mut self: Obj<Self>, chunk: Obj<ChunkVoxelData>) {
        debug_assert_eq!(chunk.world, self);

        self.chunks.remove(&chunk.pos);

        for face in BlockFace::variants() {
            let neighbor = chunk.neighbors[face];

            let Some(mut neighbor) = neighbor else {
                continue;
            };

            neighbor.neighbors[face.invert()] = None;
        }

        // Queues the component for deletion
        chunk.entity().remove::<ChunkVoxelData>();
    }

    pub fn get(&self, pos: ChunkVec) -> Option<Obj<ChunkVoxelData>> {
        self.chunks.get(&pos).copied()
    }
}

random_component!(WorldVoxelData);

#[derive(Debug)]
pub struct ChunkVoxelData {
    world: Obj<WorldVoxelData>,
    pos: ChunkVec,
    neighbors: NumEnumMap<BlockFace, Option<Obj<ChunkVoxelData>>>,
    data: Option<Box<[u16; CHUNK_VOLUME as usize]>>,
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

    pub fn initialize_data(&mut self, data: Box<[u16; CHUNK_VOLUME as usize]>) {
        debug_assert!(self.data.is_none());

        self.data = Some(data);
    }

    pub fn data(&self) -> Option<&[u16; CHUNK_VOLUME as usize]> {
        self.data.as_deref()
    }

    pub fn data_mut(&mut self) -> Option<&mut [u16; CHUNK_VOLUME as usize]> {
        self.data.as_deref_mut()
    }
}

// === Pointer === //

pub type WorldPointer = VoxelPointer<WorldVec>;
pub type EntityPointer = VoxelPointer<EntityVec>;

#[derive(Debug, Copy, Clone)]
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

    pub fn move_to(&mut self, pos: V) {
        // Update vector
        let old_chunk = self.block().chunk();
        self.pos = pos;
        let new_chunk = self.block().chunk();

        // Update chunk
        if old_chunk == new_chunk {
            // (we're still in the same chunk)
            return;
        }

        let Some(mut chunk) = self.chunk else {
            // (the chunk cache is unset and we don't want to recompute it)
            return;
        };

        self.chunk = None;

        let delta = new_chunk - old_chunk;

        for axis in Axis3::variants() {
            let delta = delta.comp(axis);

            // We can't move more than one chunk across
            if !(-1..=1).contains(&delta) {
                return;
            }

            // If the delta is zero, don't move anywhere.
            let Some(sign) = Sign::of(delta) else {
                continue;
            };

            // If the neighbor is none, our cache becomes none.
            let Some(neighbor) = chunk.neighbor(BlockFace::compose(axis, sign)) else {
                return;
            };

            chunk = neighbor;
        }

        self.chunk = Some(chunk);
    }

    pub fn move_by(&mut self, delta: V) {
        self.move_to(self.pos + delta);
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
}

impl WorldPointer {
    pub fn move_to_neighbor(&mut self, face: BlockFace) {
        // Update vector
        let old_pos = self.pos;
        self.pos += face.unit();
        let new_pos = self.pos;

        // Update chunk
        if old_pos.chunk() == new_pos.chunk() {
            // (we didn't move chunks)
            return;
        }

        if let Some(chunk) = self.chunk {
            self.chunk = chunk.neighbor(face);
        }
    }

    #[must_use]
    pub fn neighbor(mut self, face: BlockFace) -> Self {
        self.move_to_neighbor(face);
        self
    }
}
