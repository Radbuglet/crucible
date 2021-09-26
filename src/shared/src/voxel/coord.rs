//! Type-safe wrappers around [Vector3] to represent various voxel coordinate spaces.
//!
//! "Oh boy, it *sure* is fun to manually delegate methods!" - A composition enthusiast

use cgmath::{Vector3, Zero};
use std::fmt::{Debug, Formatter, Result as FmtResult};

#[rustfmt::skip]
use std::ops::{
	Add, AddAssign,
	Sub, SubAssign,
	Mul, MulAssign,
	Div, DivAssign,
	Rem, RemAssign,
	RangeInclusive,
};

macro def_coords(
	names: {
		new: $n_new:ident,
		zero: $n_zero:ident,
		scalar: $n_scalar:ident,
		raw: $n_raw:ident,
	},
	definitions: {$(
		$(#[$attr:meta])*
		$vis:vis $name:ident($comp:ty);
	)*}
) {
	$(
		#[derive(Copy, Clone,  PartialEq)]
		$(#[$attr])*
		pub struct $name {
			pub $n_raw: Vector3<$comp>,
		}

		impl Debug for $name {
			fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
				write!(f, "{}({}, {}, {})", stringify!($name), self.$n_raw.x, self.$n_raw.y, self.$n_raw.z)
			}
		}

		// === Raw-pos conversion === //

		impl $name {
			pub fn $n_zero() -> Self {
				Self::from(Vector3::zero())
			}

			pub fn $n_new(x: $comp, y: $comp, z: $comp) -> Self {
				Self::from(Vector3::new(x, y, z))
			}

			pub fn $n_scalar(comp: $comp) -> Self {
				Self::from(Vector3::new(comp, comp, comp))
			}
		}

		// From raw
		impl From<Vector3<$comp>> for $name {
			fn from($n_raw: Vector3<$comp>) -> Self {
				Self { $n_raw }
			}
		}

		// To raw
		impl From<$name> for Vector3<$comp> {
			fn from(pos: $name) -> Self {
				pos.$n_raw
			}
		}

		// === Arithmetic === //

		impl<T: Into<Self>> Add<T> for $name {
			type Output = Self;

		    fn add(self, rhs: T) -> Self::Output {
		        Self::from(self.$n_raw + rhs.into().$n_raw)
		    }
		}

		impl<T: Into<Self>> AddAssign<T> for $name {
			fn add_assign(&mut self, rhs: T) {
				*self = *self + rhs;
			}
		}

		impl<T: Into<Self>> Sub<T> for $name {
			type Output = Self;

			fn sub(self, rhs: T) -> Self::Output {
				Self::from(self.$n_raw - rhs.into().$n_raw)
			}
		}

		impl<T: Into<Self>> SubAssign<T> for $name {
			fn sub_assign(&mut self, rhs: T) {
				*self = *self - rhs;
			}
		}

		impl Mul<$comp> for $name {
			type Output = Self;

			fn mul(self, rhs: $comp) -> Self::Output {
				Self::from(self.$n_raw * rhs)
			}
		}

		impl MulAssign<$comp> for $name {
			fn mul_assign(&mut self, rhs: $comp) {
				*self = *self * rhs;
			}
		}

		impl Div<$comp> for $name {
			type Output = Self;

			fn div(self, rhs: $comp) -> Self::Output {
				Self::from(self.$n_raw / rhs)
			}
		}

		impl DivAssign<$comp> for $name {
			fn div_assign(&mut self, rhs: $comp) {
				*self = *self / rhs;
			}
		}

		impl Rem<$comp> for $name {
			type Output = Self;

			fn rem(self, rhs: $comp) -> Self::Output {
				Self::from(self.$n_raw % rhs)
			}
		}

		impl RemAssign<$comp> for $name {
			fn rem_assign(&mut self, rhs: $comp) {
				*self = *self % rhs;
			}
		}
	)*
}

def_coords! {
	names: {
		new: new,
		zero: zero,
		scalar: scalar,
		raw: raw,
	},
	definitions: {
		// === Integer === //

		#[derive(Hash, Eq)]
		pub WorldPos(i64);

		#[derive(Hash, Eq)]
		pub ChunkPos(i64);  // Technically only a i56.

		#[derive(Hash, Eq)]
		pub BlockPos(u8);

		// === Floating point === //

		pub WorldPosF(f64);
		pub ChunkPosF(f64);
		pub BlockPosF(f64);
	}
}

pub const CHUNK_EDGE: u32 = 16;
pub const CHUNK_VOLUME: u32 = CHUNK_EDGE * CHUNK_EDGE * CHUNK_EDGE;

impl WorldPos {
	pub const MIN: i64 = i64::MIN;
	pub const MAX: i64 = i64::MAX;
	pub const RANGE: RangeInclusive<i64> = Self::MIN..=Self::MAX;

	pub fn from_parts(chunk: ChunkPos, block: BlockPos) -> Self {
		debug_assert!(
			chunk.is_valid(),
			"Attempted to construct a WorldPos with an out-of-bounds ChunkPos."
		);
		debug_assert!(
			block.is_valid(),
			"Attempted to construct a WorldPos with an out-of-bounds BlockPos."
		);

		let chunk = chunk.raw.cast::<i64>().unwrap();
		let block = block.raw.cast::<i64>().unwrap();

		Self::from(chunk * CHUNK_EDGE as _ + block)
	}

	pub fn chunk(self) -> ChunkPos {
		ChunkPos::from((self / CHUNK_EDGE as _).raw.cast::<_>().unwrap())
	}

	pub fn block(self) -> BlockPos {
		BlockPos::from((self % CHUNK_EDGE as _).raw.cast::<_>().unwrap())
	}

	pub fn split(self) -> (ChunkPos, BlockPos) {
		(self.chunk(), self.block())
	}
}

impl From<ChunkPos> for WorldPos {
	fn from(chunk: ChunkPos) -> Self {
		Self::from_parts(chunk, BlockPos::zero())
	}
}

impl From<BlockPos> for WorldPos {
	fn from(block: BlockPos) -> Self {
		Self::from_parts(ChunkPos::zero(), block)
	}
}

impl ChunkPos {
	pub const MIN: i64 = (WorldPos::MIN / CHUNK_EDGE as i64) as i64;
	pub const MAX: i64 = (WorldPos::MAX / CHUNK_EDGE as i64) as i64;
	pub const RANGE: RangeInclusive<i64> = Self::MIN..=Self::MAX;

	//noinspection DuplicatedCode
	pub fn is_valid(self) -> bool {
		Self::RANGE.contains(&self.raw.x)
			&& Self::RANGE.contains(&self.raw.y)
			&& Self::RANGE.contains(&self.raw.z)
	}
}

impl From<WorldPos> for ChunkPos {
	fn from(pos: WorldPos) -> Self {
		pos.chunk()
	}
}

impl BlockPos {
	pub const MIN: u8 = 0;
	pub const MAX: u8 = (CHUNK_EDGE - 1) as u8;
	pub const RANGE: RangeInclusive<u8> = Self::MIN..=Self::MAX;

	//noinspection DuplicatedCode
	pub fn is_valid(self) -> bool {
		Self::RANGE.contains(&self.raw.x)
			&& Self::RANGE.contains(&self.raw.y)
			&& Self::RANGE.contains(&self.raw.z)
	}
}

impl From<WorldPos> for BlockPos {
	fn from(pos: WorldPos) -> Self {
		pos.block()
	}
}

// TODO: Implement for floating-point versions

// === Tests === //

#[test]
fn test() {
	// Arithmetic
	{
		let mut pos = WorldPos::new(5, 6, 2);
		pos += WorldPos::new(2, 3, 1);
		pos += Vector3::new(5, 3, 1);
		pos += ChunkPos::new(1, 1, 1);
		pos += BlockPos::new(0, 2, 0);
		assert_eq!(pos, WorldPos::new(28, 30, 20));
	}

	// Validity
	assert_eq!(
		WorldPos::scalar(WorldPos::MAX),
		WorldPos::from_parts(
			ChunkPos::scalar(ChunkPos::MAX),
			BlockPos::scalar(BlockPos::MAX),
		),
	)
}
