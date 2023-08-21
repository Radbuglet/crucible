use std::f32::consts::{PI, TAU};

use crucible_util::{c_enum, mem::c_enum::CEnum};
use num_traits::Signed;
use typed_glam::{
	glam::{DVec2, DVec3, IVec2, IVec3, Mat4, Vec2, Vec3},
	traits::{NumericVector, NumericVector2, NumericVector3, SignedNumericVector3},
	typed::{FlavorCastFrom, TypedVector, VecFlavor},
};

// === Sign === //

c_enum! {
	pub enum Sign {
		Positive,
		Negative,
	}
}

impl Sign {
	pub fn is_negative(self) -> bool {
		matches!(self, Self::Negative)
	}

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

	pub fn unit_typed<T: Signed>(self) -> T {
		use Sign::*;

		match self {
			Positive => T::one(),
			Negative => -T::one(),
		}
	}

	pub fn unit_i32(self) -> i32 {
		self.unit_typed()
	}

	pub fn unit_i64(self) -> i64 {
		self.unit_typed()
	}

	pub fn unit_f32(self) -> f32 {
		self.unit_typed()
	}

	pub fn unit_f64(self) -> f64 {
		self.unit_typed()
	}
}

// === Axis2 === //

c_enum! {
	pub enum Axis2 {
		X,
		Y,
	}
}

impl Axis2 {
	pub fn unit_i(self) -> IVec2 {
		self.unit_typed()
	}

	pub fn unit_f(self) -> Vec2 {
		self.unit_typed()
	}

	pub fn unit_d(self) -> DVec2 {
		self.unit_typed()
	}

	pub fn unit_typed<V: NumericVector2>(self) -> V {
		use Axis2::*;

		match self {
			X => V::X,
			Y => V::Y,
		}
	}
}

// === Axis3 === //

c_enum! {
	pub enum Axis3 {
		X,
		Y,
		Z,
	}
}

impl Axis3 {
	pub fn unit_i(self) -> IVec3 {
		self.unit_typed()
	}

	pub fn unit_f(self) -> Vec3 {
		self.unit_typed()
	}

	pub fn unit_d(self) -> DVec3 {
		self.unit_typed()
	}

	pub fn unit_typed<V: NumericVector3>(self) -> V {
		use Axis3::*;

		match self {
			X => V::X,
			Y => V::Y,
			Z => V::Z,
		}
	}

	pub fn ortho_hv(self) -> (Self, Self) {
		match self {
			// As a reminder, our coordinate system is y-up right-handed and looks like this:
			//
			//     +y
			//      |
			// +x---|
			//     /
			//   +z
			//
			Self::X => {
				// A quad facing the negative x direction looks like this:
				//
				//       c +y
				//      /|
				//     / |
				//    d  |
				//    |  b 0
				//    | /     ---> -x
				//    |/
				//    a +z
				//
				(Self::Z, Self::Y)
			}
			Self::Y => {
				// A quad facing the negative y direction looks like this:
				//
				//  +x        0
				//    d------a    |
				//   /      /     |
				//  /      /      ↓ -y
				// c------b
				//         +z
				(Self::X, Self::Z)
			}
			Self::Z => {
				// A quad facing the negative z direction looks like this:
				//
				//              +y
				//      c------d
				//      |      |     ^ -z
				//      |      |    /
				//      b------a   /
				//    +x        0
				//
				(Self::X, Self::Y)
			}
		}
	}

	pub fn extrude_volume_hv<V: NumericVector3>(
		self,
		size: impl Into<(V::Comp, V::Comp)>,
		perp: V::Comp,
	) -> V {
		let (ha, va) = self.ortho_hv();
		let (hm, vm) = size.into();

		let mut target = V::ZERO;
		*target.comp_mut(ha) = hm;
		*target.comp_mut(va) = vm;
		*target.comp_mut(self) = perp;

		target
	}
}

// === Axis Extensions === //

pub trait VecCompExt<A: CEnum>: NumericVector {
	fn comp(&self, axis: A) -> Self::Comp {
		self[axis.index()]
	}

	fn comp_mut(&mut self, axis: A) -> &mut Self::Comp {
		&mut self[axis.index()]
	}
}

impl<V: NumericVector2> VecCompExt<Axis2> for V {}

impl<V: NumericVector3> VecCompExt<Axis3> for V {}

// === Angle3D === //

pub const HALF_PI: f32 = PI / 2.0;

pub type Angle3D = TypedVector<Angle3DFlavor>;

pub struct Angle3DFlavor;

impl VecFlavor for Angle3DFlavor {
	type Backing = Vec2;

	const DEBUG_NAME: &'static str = "Angle3D";
}

impl FlavorCastFrom<Vec2> for Angle3DFlavor {
	fn cast_from(vec: Vec2) -> TypedVector<Self>
	where
		Self: VecFlavor,
	{
		TypedVector::from_glam(vec)
	}
}

pub trait Angle3DExt {
	#[must_use]
	fn new_deg(yaw: f32, pitch: f32) -> Self;

	#[must_use]
	fn as_matrix(&self) -> Mat4;

	#[must_use]
	fn as_matrix_horizontal(&self) -> Mat4;

	#[must_use]
	fn as_matrix_vertical(&self) -> Mat4;

	#[must_use]
	fn forward(&self) -> Vec3;

	#[must_use]
	fn wrap(&self) -> Self;

	#[must_use]
	fn wrap_x(&self) -> Self;

	#[must_use]
	fn wrap_y(&self) -> Self;

	#[must_use]
	fn clamp_y(&self, min: f32, max: f32) -> Self;

	#[must_use]
	fn clamp_y_90(&self) -> Self;
}

impl Angle3DExt for Angle3D {
	fn new_deg(yaw: f32, pitch: f32) -> Self {
		Self::new(yaw.to_radians(), pitch.to_radians())
	}

	fn as_matrix(&self) -> Mat4 {
		self.as_matrix_horizontal() * self.as_matrix_vertical()
	}

	fn as_matrix_horizontal(&self) -> Mat4 {
		Mat4::from_rotation_y(self.x())
	}

	fn as_matrix_vertical(&self) -> Mat4 {
		Mat4::from_rotation_x(self.y())
	}

	fn forward(&self) -> Vec3 {
		self.as_matrix().transform_vector3(Vec3::Z)
	}

	fn wrap(&self) -> Self {
		self.wrap_x().wrap_y()
	}

	fn wrap_x(&self) -> Self {
		Self::new(self.x().rem_euclid(TAU), self.y())
	}

	fn wrap_y(&self) -> Self {
		Self::new(self.x(), self.y().rem_euclid(TAU))
	}

	fn clamp_y(&self, min: f32, max: f32) -> Self {
		Self::new(self.x(), self.y().clamp(min, max))
	}

	fn clamp_y_90(&self) -> Self {
		self.clamp_y(-HALF_PI, HALF_PI)
	}
}

// === Misc Math === //

pub fn lerp_percent_at(val: f64, start: f64, end: f64) -> f64 {
	// start + (end - start) * percent = val
	// (val - start) / (end - start) = percent
	(val - start) / (end - start)
}

// === BlockFace === //

c_enum! {
	pub enum BlockFace {
		PositiveX,
		NegativeX,
		PositiveY,
		NegativeY,
		PositiveZ,
		NegativeZ,
	}
}

impl BlockFace {
	pub const TOP: Self = Self::PositiveY;

	pub const BOTTOM: Self = Self::NegativeY;

	pub const SIDES: [Self; 4] = [
		Self::PositiveX,
		Self::NegativeZ,
		Self::NegativeX,
		Self::PositiveZ,
	];

	pub fn from_vec(vec: IVec3) -> Option<Self> {
		let mut choice = None;

		for axis in Axis3::variants() {
			let comp = vec.comp(axis);

			if comp != 0 && choice.is_some() {
				return None;
			}

			if comp.abs() == 1 {
				choice = Some(BlockFace::compose(axis, Sign::of(comp).unwrap()));
			}
		}

		choice
	}

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
		self.unit_typed()
	}

	pub fn unit_typed<V>(self) -> V
	where
		V: SignedNumericVector3,
	{
		let v = self.axis().unit_typed::<V>();
		if self.sign() == Sign::Negative {
			-v
		} else {
			v
		}
	}
}

// === f32 utils === //

/// How many bits are actually used to store a mantissa.
///
/// Because the leading digit of our mantissa is always an implied one, this is just one less than
/// [`f32::MANTISSA_DIGITS`], which measures the logical number of digits.
pub const MANTISSA_BITS: u32 = f32::MANTISSA_DIGITS - 1;

/// The maximum value a mantissa could be when represented as a pure `u32`.
pub const MAX_MANTISSA_EXCLUSIVE: u32 = 1 << MANTISSA_BITS;

/// A bitmask for the bits used in a floating-point mantissa.
pub const MANTISSA_MASK: u32 = MAX_MANTISSA_EXCLUSIVE - 1;

/// The floating-point exponent for the range `0` to `1`.
pub const ZERO_TO_ONE_EXPONENT: u8 = 0b01111110;

pub fn compose_f32(sign: Sign, exp: u8, mantissa: u32) -> f32 {
	debug_assert!(mantissa < MAX_MANTISSA_EXCLUSIVE);
	let bits = mantissa +  // Bits 0 to `MANTISSA_DIGITS - 2`
		((exp as u32) << MANTISSA_BITS) +  // Bits `MANTISSA_BITS` to `MANTISSA_BITS + 8`
		((sign.is_negative() as u32) << 31); // Bit 31

	f32::from_bits(bits)
}
