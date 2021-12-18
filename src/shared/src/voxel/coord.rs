//! Type-safe wrappers around [Vector3] to represent various voxel coordinate spaces.
//!
//! "Oh boy, it *sure* is fun to manually delegate methods!" - A composition enthusiast

// TODO: Document coordinate spaces.

use cgmath::{num_traits::Signed, BaseNum, Vector3, Zero};
use crucible_core::util::meta_enum::{enum_meta, EnumMeta};
use std::fmt::{Debug, Formatter, Result as FmtResult};
use std::ops::{Deref, Neg};

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
		#[derive(Hash, Eq)]
		pub WorldPos(i64);

		#[derive(Hash, Eq)]
		pub ChunkPos(i64);  // Technically only a i56.

		#[derive(Hash, Eq)]
		pub BlockPos(u8);
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

	pub fn to_index(self) -> usize {
		let raw = self.raw.cast::<u32>().unwrap();
		(raw.x + raw.y * CHUNK_EDGE + raw.z * CHUNK_EDGE * CHUNK_EDGE) as usize
	}

	pub fn from_index(index: usize) -> Self {
		let mut index = index as u32;
		let x = (index % CHUNK_EDGE) as u8;
		index /= CHUNK_EDGE;
		let y = (index % CHUNK_EDGE) as u8;
		index /= CHUNK_EDGE;
		let z = (index % CHUNK_EDGE) as u8;

		Vector3::new(x, y, z).into()
	}
}

impl From<WorldPos> for BlockPos {
	fn from(pos: WorldPos) -> Self {
		pos.block()
	}
}

// TODO: Floating-point world and block positions

// === Block faces === //

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum Sign {
	Positive,
	Negative,
}

enum_meta! {
	#[derive(Debug)]
	pub enum(Axis3Meta) Axis3 {
		X = Axis3Meta {
			vec_idx: 0,
			ortho: [Axis3::Y, Axis3::Z],
		},
		Y = Axis3Meta {
			vec_idx: 1,
			ortho: [Axis3::Z, Axis3::X],
		},
		Z = Axis3Meta {
			vec_idx: 2,
			ortho: [Axis3::X, Axis3::Y],
		}
	}

	#[derive(Debug)]
	pub enum(BlockFaceMeta) BlockFace {
		Px = BlockFaceMeta {
			axis: Axis3::X,
			sign: Sign::Positive,
			inverse: BlockFace::Nx,
		},
		Py = BlockFaceMeta {
			axis: Axis3::Y,
			sign: Sign::Positive,
			inverse: BlockFace::Ny,
		},
		Pz = BlockFaceMeta {
			axis: Axis3::Z,
			sign: Sign::Positive,
			inverse: BlockFace::Nz,
		},
		Nx = BlockFaceMeta {
			axis: Axis3::X,
			sign: Sign::Negative,
			inverse: BlockFace::Px,
		},
		Ny = BlockFaceMeta {
			axis: Axis3::Y,
			sign: Sign::Negative,
			inverse: BlockFace::Py,
		},
		Nz = BlockFaceMeta {
			axis: Axis3::Z,
			sign: Sign::Negative,
			inverse: BlockFace::Pz,
		}
	}
}

pub struct Axis3Meta {
	/// The index of the axis in a [Vector3].
	pub vec_idx: usize,

	/// Orthogonal axes sorted in CCW order assuming a viewing direction looking at the face from
	/// its positive side.
	pub ortho: [Axis3; 2],
}

pub struct BlockFaceMeta {
	pub sign: Sign,
	pub axis: Axis3,
	pub inverse: BlockFace,
}

impl Sign {
	pub fn of<N: Signed>(other: N) -> Option<Self> {
		match other {
			v if v.is_positive() => Some(Sign::Positive),
			v if v.is_negative() => Some(Sign::Negative),
			_ => None,
		}
	}

	pub fn of_poszero<N: Signed>(other: N) -> Self {
		Self::of(other).unwrap_or(Sign::Positive)
	}

	pub fn unit<N: Signed>(self) -> N {
		match self {
			Sign::Positive => N::one(),
			Sign::Negative => -N::one(),
		}
	}
}

impl Neg for Sign {
	type Output = Self;

	fn neg(self) -> Self::Output {
		match self {
			Sign::Positive => Sign::Negative,
			Sign::Negative => Sign::Positive,
		}
	}
}

impl Axis3 {
	pub const COUNT: usize = 3;

	pub fn unit<N: BaseNum>(self) -> Vector3<N> {
		let mut vec = Vector3::zero();
		vec[self.vec_idx] = N::one();
		vec
	}

	pub fn pos_face(self) -> BlockFace {
		BlockFace::from(self, Sign::Positive)
	}

	pub fn neg_face(self) -> BlockFace {
		BlockFace::from(self, Sign::Negative)
	}
}

impl Deref for Axis3 {
	type Target = Axis3Meta;

	fn deref(&self) -> &Self::Target {
		self.meta()
	}
}

impl BlockFace {
	pub const COUNT: usize = 6;

	pub fn marshall_shader(self) -> u32 {
		self as u32
	}

	pub fn from(axis: Axis3, sign: Sign) -> Self {
		Self::values_iter()
			.find_map(|(face, meta)| {
				if meta.axis == axis && meta.sign == sign {
					Some(face)
				} else {
					None
				}
			})
			.unwrap()
	}

	pub fn unit<N>(self) -> Vector3<N>
	where
		N: BaseNum + Signed,
	{
		let BlockFaceMeta { axis, sign, .. } = &*self;
		axis.unit() * sign.unit()
	}

	pub fn ortho_ccw<N: BaseNum + Signed + Copy>(self) -> [Vector3<N>; 2] {
		let (a, b) = match self.sign {
			Sign::Positive => (0, 1),
			Sign::Negative => (1, 0),
		};

		[
			self.axis.ortho[a].pos_face().unit(),
			self.axis.ortho[b].pos_face().unit(),
		]
	}

	pub fn quad_ccw<N: BaseNum + Signed + Copy>(self) -> [Vector3<N>; 4] {
		let corner = match self.sign {
			Sign::Positive => self.axis.unit(),
			Sign::Negative => Vector3::zero(),
		};
		let [a, b] = self.ortho_ccw();
		[corner, corner + a, corner + a + b, corner + b]
	}
}

impl Neg for BlockFace {
	type Output = Self;

	fn neg(self) -> Self::Output {
		Self::from(self.axis, -self.sign)
	}
}

impl Deref for BlockFace {
	type Target = BlockFaceMeta;

	fn deref(&self) -> &Self::Target {
		self.meta()
	}
}

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
