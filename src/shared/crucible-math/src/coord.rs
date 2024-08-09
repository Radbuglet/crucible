//! ## Coordinate System
//!
//! Crucible uses a **y-up right-handed** coordinate system. Thus, our axes look like this:
//!
//! ```plain_text
//!     +y
//!      |
//! +x---|
//!     /
//!   +z
//! ```
//!
//! This coordinate system is nice because it works well with graphics conventions. For example,
//! because object depth increases along the positive `z` direction, camera view matrices transform
//! positive `z` local-space vectors into the forward direction. Thus, it is fair to call:
//!
//! ```plain_text
//!              +y (up)
//!              | +z (forward)
//!              |/
//! (left) -x----|---+x (right)
//!             /|
//!            / |
//!    (back) -z  -y (down)
//! ```
//!
//! ...which feels pretty intuitive.
//!
//! ## Relation to Voxels
//!
//! There are four major typed vector types to represent Crucible's various coordinate systems:
//!
//! 1. [`WorldVec`]: a block vector in world-space i.e. global block coordinates.
//! 2. [`ChunkVec`]: a chunk position vector i.e. the coordinate of a chunk.
//! 3. [`BlockVec`]: a block vector in chunk-relative-space i.e. the coordinate of a block relative
//!    to a chunk.
//! 4. [`EntityVec`]: an entity vector in world-space i.e. global entity coordinates.
//!
//! ##### `EntityVec` and `WorldVec`
//!
//! A voxel takes up the size `EntityVec::ONE`. For a given voxel at `(x, y, z)` in world-vec
//! coordinates, the corresponding entity-vec at `(x, y, z)` will be positioned at the bottom most-
//! negative corner of the block:
//!
//! ```plain_text
//!              <1>
//!          *---------*    +y
//!         /         /|    |  +z
//!        / |       / |    | /
//!       *---------|  |    |/
//!       |  /- - - | -*    *-----+x
//!   <1> | /       | /
//!       |/        |/    <----- voxel at (x, y, z)
//!       #---------*
//!       ^   <1>
//!       |---- point at (x, y, z)
//! ```
//!
//! ##### `BlockVec`, `ChunkVec`, and `WorldVec`
//!
//! A chunk is a cubic section of `(CHUNK_EDGE, CHUNK_EDGE, CHUNK_EDGE)` blocks. Like entity
//! coordinates on a block, chunk coordinates, when converted to world block vectors, correspond to
//! the negative-most corner of the chunk. `BlockVec` measure block positions relative to that point.
//!
//! A valid `BlockVec` is comprised of components from 0 to `CHUNK_EDGE` upper exclusive.
//!
//! ## Block Faces
//!
//! Block faces are axis-aligned, and are enumerated by the `BlockFace` enum.

use typed_glam::{
    ext::VecExt,
    glam::{self, DVec3},
    typed::{FlavorCastFrom, TypedVector, VecFlavor},
};

use crate::{AaPlane, BlockFace, EntityAabb, Sign, VecCompExt};

// === Constants === //

pub type BlockIndex = u16;

const __ASSERT: () = assert!(CHUNK_VOLUME <= BlockIndex::MAX as i32);

pub const CHUNK_EDGE: i32 = 16;
pub const CHUNK_LAYER: i32 = CHUNK_EDGE.pow(2);
pub const CHUNK_VOLUME: i32 = CHUNK_EDGE.pow(3);

// === `WorldVec` === //

pub type WorldVec = TypedVector<WorldVecFlavor>;

#[non_exhaustive]
pub struct WorldVecFlavor;

impl VecFlavor for WorldVecFlavor {
    type Backing = glam::IVec3;

    const DEBUG_NAME: &'static str = "WorldVec";
}

impl FlavorCastFrom<glam::IVec3> for WorldVecFlavor {
    fn cast_from(v: glam::IVec3) -> WorldVec {
        WorldVec::from_glam(v)
    }
}

impl FlavorCastFrom<i32> for WorldVecFlavor {
    fn cast_from(v: i32) -> WorldVec {
        WorldVec::splat(v)
    }
}

impl FlavorCastFrom<EntityVec> for WorldVecFlavor {
    fn cast_from(vec: EntityVec) -> TypedVector<Self> {
        vec.block_pos()
    }
}

pub trait WorldVecExt: Sized {
    fn compose(chunk: ChunkVec, block: BlockVec) -> Self;
    fn decompose(self) -> (ChunkVec, BlockVec);

    fn chunk(self) -> ChunkVec;
    fn block(self) -> BlockVec;
    fn negative_most_corner(self) -> EntityVec;
    fn full_aabb(self) -> EntityAabb;

    fn face_plane(self, face: BlockFace) -> AaPlane;
}

impl WorldVecExt for WorldVec {
    fn compose(chunk: ChunkVec, block: BlockVec) -> Self {
        debug_assert!(chunk.is_valid());
        debug_assert!(block.is_valid());
        Self::from_glam(chunk.to_glam() * CHUNK_EDGE + block.to_glam())
    }

    fn decompose(self) -> (ChunkVec, BlockVec) {
        (self.chunk(), self.block())
    }

    fn chunk(self) -> ChunkVec {
        ChunkVec::new(
            self.x().div_euclid(CHUNK_EDGE),
            self.y().div_euclid(CHUNK_EDGE),
            self.z().div_euclid(CHUNK_EDGE),
        )
    }

    fn block(self) -> BlockVec {
        BlockVec::new(
            self.x().rem_euclid(CHUNK_EDGE),
            self.y().rem_euclid(CHUNK_EDGE),
            self.z().rem_euclid(CHUNK_EDGE),
        )
    }

    fn negative_most_corner(self) -> EntityVec {
        self.map_glam(|raw| raw.as_dvec3())
    }

    fn full_aabb(self) -> EntityAabb {
        EntityAabb {
            origin: self.negative_most_corner(),
            size: EntityVec::ONE,
        }
    }

    fn face_plane(self, face: BlockFace) -> AaPlane {
        let corner = self.negative_most_corner();
        let (axis, sign) = face.decompose();

        AaPlane {
            origin: if sign == Sign::Positive {
                corner.comp(axis) + 1.
            } else {
                corner.comp(axis)
            },
            normal: face,
        }
    }
}

// === `ChunkVec` === //

pub type ChunkVec = TypedVector<ChunkVecFlavor>;

#[non_exhaustive]
pub struct ChunkVecFlavor;

impl VecFlavor for ChunkVecFlavor {
    type Backing = glam::IVec3;

    const DEBUG_NAME: &'static str = "ChunkVec";
}

impl FlavorCastFrom<glam::IVec3> for ChunkVecFlavor {
    fn cast_from(v: glam::IVec3) -> ChunkVec {
        ChunkVec::from_glam(v)
    }
}

impl FlavorCastFrom<i32> for ChunkVecFlavor {
    fn cast_from(v: i32) -> ChunkVec {
        ChunkVec::splat(v)
    }
}

pub trait ChunkVecExt: Sized {
    fn is_valid(&self) -> bool;
}

impl ChunkVecExt for ChunkVec {
    fn is_valid(&self) -> bool {
        self.all(|comp| comp.checked_mul(CHUNK_EDGE).is_some())
    }
}

// === `BlockVec` === //

pub type BlockVec = TypedVector<BlockVecFlavor>;

#[non_exhaustive]
pub struct BlockVecFlavor;

impl VecFlavor for BlockVecFlavor {
    type Backing = glam::IVec3;

    const DEBUG_NAME: &'static str = "BlockVec";
}

impl FlavorCastFrom<glam::IVec3> for BlockVecFlavor {
    fn cast_from(v: glam::IVec3) -> BlockVec {
        BlockVec::from_glam(v)
    }
}

impl FlavorCastFrom<i32> for BlockVecFlavor {
    fn cast_from(v: i32) -> BlockVec {
        BlockVec::splat(v)
    }
}

pub trait BlockVecExt: Sized {
    fn is_valid(&self) -> bool;
    fn wrap(self) -> Self;
    fn iter() -> BlockPosIter;

    fn to_index(self) -> usize;
    fn try_from_index(index: usize) -> Option<Self>;
    fn from_index(index: usize) -> Self;
    fn is_valid_index(index: usize) -> bool;
}

impl BlockVecExt for BlockVec {
    fn is_valid(&self) -> bool {
        self.all(|comp| (0..CHUNK_EDGE).contains(&comp))
    }

    fn wrap(self) -> Self {
        self.map(|comp| comp.rem_euclid(CHUNK_EDGE))
    }

    fn iter() -> BlockPosIter {
        BlockPosIter(0)
    }

    fn to_index(self) -> usize {
        debug_assert!(self.is_valid());
        (self.x() + self.y() * CHUNK_EDGE + self.z() * CHUNK_LAYER) as usize
    }

    fn try_from_index(index: usize) -> Option<Self> {
        if Self::is_valid_index(index) {
            Some(Self::from_index(index))
        } else {
            None
        }
    }

    fn from_index(index: usize) -> Self {
        debug_assert!(Self::is_valid_index(index));

        let mut index = index as i32;
        let x = index % CHUNK_EDGE;
        index /= CHUNK_EDGE;
        let y = index % CHUNK_EDGE;
        index /= CHUNK_EDGE;
        let z = index % CHUNK_EDGE;

        Self::new(x, y, z)
    }

    fn is_valid_index(index: usize) -> bool {
        index < CHUNK_VOLUME as usize
    }
}

#[derive(Debug)]
pub struct BlockPosIter(usize);

impl Iterator for BlockPosIter {
    type Item = BlockVec;

    fn next(&mut self) -> Option<Self::Item> {
        let pos = BlockVec::try_from_index(self.0)?;
        self.0 += 1;
        Some(pos)
    }
}

// === `EntityVec` === //

/// A vector in the logical vector-space of valid entity positions. This is a double precision float
/// vector because we need all world positions to be encodable as entity positions.
pub type EntityVec = TypedVector<EntityVecFlavor>;

#[non_exhaustive]
pub struct EntityVecFlavor;

impl VecFlavor for EntityVecFlavor {
    type Backing = DVec3;

    const DEBUG_NAME: &'static str = "EntityVec";
}

impl FlavorCastFrom<glam::Vec3> for EntityVecFlavor {
    fn cast_from(v: glam::Vec3) -> EntityVec {
        EntityVec::from_glam(v.as_dvec3())
    }
}

impl FlavorCastFrom<glam::DVec3> for EntityVecFlavor {
    fn cast_from(v: glam::DVec3) -> EntityVec {
        EntityVec::from_glam(v)
    }
}

impl FlavorCastFrom<f64> for EntityVecFlavor {
    fn cast_from(v: f64) -> EntityVec {
        EntityVec::splat(v)
    }
}

impl FlavorCastFrom<WorldVec> for EntityVecFlavor {
    fn cast_from(v: WorldVec) -> EntityVec {
        v.negative_most_corner()
    }
}

pub trait EntityVecExt {
    const HORIZONTAL: Self;

    fn block_pos(self) -> WorldVec;
}

impl EntityVecExt for EntityVec {
    const HORIZONTAL: Self = Self::from_glam(DVec3::new(1.0, 0.0, 1.0));

    fn block_pos(self) -> WorldVec {
        self.map_glam(|raw| raw.floor().as_ivec3())
    }
}
