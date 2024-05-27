use bevy_autoken::{random_component, Obj, RandomEntityExt};
use bevy_ecs::entity::Entity;
use crucible_math::{BlockFace, ChunkVec, CHUNK_VOLUME};
use newtypes::{NumEnum, NumEnumMap};
use rustc_hash::FxHashMap;

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
