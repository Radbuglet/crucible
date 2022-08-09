use num_traits::Signed;
use typed_glam::{
	ext::VecExt,
	glam::{self, DVec3, IVec3},
	traits::NumericVector3,
	typed::{FlavorCastFrom, TypedVector, VecFlavor},
};

use crucible_core::c_enum::{c_enum, CEnum};

// === Coordinate Systems === //

pub const CHUNK_EDGE: i32 = 16;
pub const CHUNK_LAYER: i32 = CHUNK_EDGE.pow(2);
pub const CHUNK_VOLUME: i32 = CHUNK_EDGE.pow(3);

// === `WorldVec` === //

pub type WorldVec = TypedVector<WorldVecFlavor>;

pub struct WorldVecFlavor(!);

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

pub trait WorldVecExt: Sized {
	fn compose(chunk: ChunkVec, block: BlockVec) -> Self;
	fn decompose(self) -> (ChunkVec, BlockVec);
	fn chunk(self) -> ChunkVec;
	fn block(self) -> BlockVec;
	fn min_corner_pos(self) -> EntityVec;
	fn block_interface_layer(self, face: BlockFace) -> f64;
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

	fn min_corner_pos(self) -> EntityVec {
		self.map_glam(|raw| raw.as_dvec3())
	}

	fn block_interface_layer(self, face: BlockFace) -> f64 {
		let corner = self.min_corner_pos();
		let (axis, sign) = face.decompose();

		if sign == Sign::Positive {
			corner.comp(axis) + 1.
		} else {
			corner.comp(axis)
		}
	}
}

// === `ChunkVec` === //

pub type ChunkVec = TypedVector<ChunkVecFlavor>;

pub struct ChunkVecFlavor(!);

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

pub struct BlockVecFlavor(!);

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
		self.all(|comp| comp >= 0 && comp < CHUNK_EDGE)
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
///
/// ## Conventions
///
/// A block position, when converted to a floating point, represents the most negative corner of a
/// given block. For example, the block position `(-2, -3, 1)` occupies the space
/// `(-2..-1, -3..-2, 1..2)`.
pub type EntityVec = TypedVector<EntityVecFlavor>;

pub struct EntityVecFlavor(!);

impl VecFlavor for EntityVecFlavor {
	type Backing = DVec3;

	const DEBUG_NAME: &'static str = "EntityVec";
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

pub trait EntityVecExt {
	fn block_pos(self) -> WorldVec;
}

impl EntityVecExt for EntityVec {
	fn block_pos(self) -> WorldVec {
		self.map_glam(|raw| raw.floor().as_ivec3())
	}
}

// === Enums === //

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

	pub fn unit(self) -> IVec3 {
		self.axis().unit() * self.sign().unit::<i32>()
	}

	pub fn ortho(self) -> (Self, Self) {
		let sign = self.sign();

		// Get axes with proper winding
		let (a, b) = if sign == Sign::Positive {
			self.axis().ortho()
		} else {
			let (a, b) = self.axis().ortho();
			(b, a)
		};

		// Construct faces
		(Self::compose(a, sign), Self::compose(b, sign))
	}
}

// Axis3
impl Axis3 {
	pub fn unit(self) -> IVec3 {
		use Axis3::*;

		match self {
			X => IVec3::X,
			Y => IVec3::Y,
			Z => IVec3::Z,
		}
	}
}

impl Axis3 {
	pub fn ortho(self) -> (Self, Self) {
		match self {
			Self::X => (Self::Z, Self::Y),
			Self::Y => (Self::X, Self::Z),
			Self::Z => (Self::Y, Self::X),
		}
	}

	pub fn plane_intersect(self, layer: f64, line: Line3) -> (f64, EntityVec) {
		let lerp = lerp_percent_at(layer, line.start.comp(self), line.end.comp(self));
		(lerp, line.start.lerp(line.end, lerp))
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

// === Line3 === //

#[derive(Debug, Copy, Clone)]
pub struct Line3 {
	pub start: EntityVec,
	pub end: EntityVec,
}

impl Line3 {
	pub fn new_origin_delta(start: EntityVec, delta: EntityVec) -> Self {
		Self {
			start: start,
			end: start + delta,
		}
	}
}

// === Misc Math === //

pub fn lerp_percent_at(val: f64, start: f64, end: f64) -> f64 {
	// start + (end - start) * percent = val
	// (val - start) / (end - start) = percent
	(val - start) / (end - start)
}

// === Vector extensions === //

pub trait Vec3Ext: NumericVector3 {
	fn comp(&self, axis: Axis3) -> Self::Comp;

	fn comp_mut(&mut self, axis: Axis3) -> &mut Self::Comp;
}

impl<V: NumericVector3> Vec3Ext for V {
	fn comp(&self, axis: Axis3) -> Self::Comp {
		self[axis.index()]
	}

	fn comp_mut(&mut self, axis: Axis3) -> &mut Self::Comp {
		&mut self[axis.index()]
	}
}
