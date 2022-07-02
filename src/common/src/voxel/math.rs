// TODO: The types of numbers in this module are completely messed up!

use std::ops::Index;

use cgmath::{num_traits::Signed, vec3, BaseNum, Vector3};

use crate::polyfill::c_enum::{c_enum, ExposesVariants};

// === Coordinate Systems === //

pub const CHUNK_EDGE: u8 = 16;
pub const CHUNK_EDGE_USIZE: usize = CHUNK_EDGE as usize;
pub const CHUNK_LAYER: usize = CHUNK_EDGE_USIZE.pow(2);
pub const CHUNK_VOLUME: usize = CHUNK_EDGE_USIZE.pow(3);

pub type WorldPos = Vector3<i64>;
pub type ChunkPos = Vector3<i64>;
pub type BlockPos = Vector3<u8>;

pub fn is_valid_chunk_pos(pos: ChunkPos) -> bool {
	Axis3::variants().all(|comp| pos[comp].checked_mul(CHUNK_EDGE as i64).is_some())
}

pub fn is_valid_block_pos(pos: BlockPos) -> bool {
	Axis3::variants().all(|comp| pos[comp] <= CHUNK_EDGE)
}

pub fn is_valid_block_index(index: usize) -> bool {
	index < CHUNK_VOLUME
}

pub fn chunk_pos_of(pos: WorldPos) -> ChunkPos {
	pos.map(|val| val.div_euclid(CHUNK_EDGE.into()))
}

pub fn block_pos_of(pos: WorldPos) -> BlockPos {
	pos.map(|val| val.rem_euclid(CHUNK_EDGE.into()) as u8)
}

pub fn decompose_world_pos(pos: WorldPos) -> (ChunkPos, BlockPos) {
	(chunk_pos_of(pos), block_pos_of(pos))
}

pub fn compose_world_pos(chunk: ChunkPos, block: BlockPos) -> WorldPos {
	debug_assert!(is_valid_chunk_pos(chunk));
	debug_assert!(is_valid_block_pos(block));

	chunk * CHUNK_EDGE as i64 + block.map(|v| v as i64)
}

pub fn block_pos_to_index(pos: BlockPos) -> usize {
	debug_assert!(is_valid_block_pos(pos));
	let pos = pos.map(|v| v as usize);
	pos.x + pos.y * CHUNK_EDGE_USIZE + pos.y * CHUNK_LAYER
}

pub fn index_to_block_pos(mut index: usize) -> BlockPos {
	debug_assert!(is_valid_block_index(index));

	let x = chunk_edge_rem_usize(index);
	index /= CHUNK_EDGE_USIZE;

	let y = chunk_edge_rem_usize(index);
	index /= CHUNK_EDGE_USIZE;

	let z = chunk_edge_rem_usize(index);

	vec3(x, y, z)
}

pub fn chunk_edge_rem_usize(val: usize) -> u8 {
	(val % CHUNK_EDGE_USIZE) as u8
}

pub fn iter_chunk_blocks() -> impl Iterator<Item = (usize, BlockPos)> {
	(0..CHUNK_VOLUME).map(|index| (index, index_to_block_pos(index)))
}

// === Block Faces === //

c_enum! {
	pub enum BlockFace {
		PositiveX,
		NegativeX,
		PositiveY,
		NegativeY,
		PositiveZ,
		NegativeZ,
	}

	pub enum Axis3 {
		X,
		Y,
		Z,
	}

	pub enum Sign {
		Positive,
		Negative,
	}
}

// BlockFace
impl BlockFace {
	pub fn compose(axis: Axis3, sign: Sign) -> Self {
		use Axis3::*;
		use BlockFace::*;
		use Sign::*;

		match (axis, sign) {
			(X, Positive) => PositiveX,
			(X, Negative) => NegativeX,
			(Y, Positive) => PositiveY,
			(Y, Negative) => NegativeY,
			(Z, Positive) => PositiveZ,
			(Z, Negative) => NegativeZ,
		}
	}

	pub fn decompose(self) -> (Axis3, Sign) {
		(self.axis(), self.sign())
	}

	pub fn axis(self) -> Axis3 {
		use Axis3::*;
		use BlockFace::*;

		match self {
			PositiveX => X,
			NegativeX => X,
			PositiveY => Y,
			NegativeY => Y,
			PositiveZ => Z,
			NegativeZ => Z,
		}
	}

	pub fn sign(self) -> Sign {
		use BlockFace::*;
		use Sign::*;

		match self {
			PositiveX => Positive,
			NegativeX => Negative,
			PositiveY => Positive,
			NegativeY => Negative,
			PositiveZ => Positive,
			NegativeZ => Negative,
		}
	}

	pub fn invert(self) -> Self {
		Self::compose(self.axis(), self.sign().invert())
	}

	pub fn unit<T: BaseNum + Signed>(self) -> Vector3<T> {
		self.axis().unit() * self.sign().unit()
	}
}

// Axis3
impl Axis3 {
	pub fn unit<T: BaseNum>(self) -> Vector3<T> {
		use Axis3::*;

		match self {
			X => Vector3::unit_x(),
			Y => Vector3::unit_y(),
			Z => Vector3::unit_z(),
		}
	}
}

impl<T> Index<Axis3> for Vector3<T> {
	type Output = T;

	fn index(&self, index: Axis3) -> &Self::Output {
		&self[index as usize]
	}
}

// Sign
impl Sign {
	pub fn of<T: Signed>(val: T) -> Option<Self> {
		if val.is_positive() {
			Some(Self::Positive)
		} else if val.is_negative() {
			Some(Self::Negative)
		} else {
			None
		}
	}

	pub fn invert(self) -> Self {
		use Sign::*;

		match self {
			Positive => Negative,
			Negative => Positive,
		}
	}

	pub fn unit<T: Signed>(self) -> T {
		use Sign::*;

		match self {
			Positive => T::one(),
			Negative => -T::one(),
		}
	}
}
