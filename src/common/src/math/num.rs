use std::cmp::Ordering;
use std::fmt::{Display, Formatter};
use std::ops::{Add, AddAssign, Mul, MulAssign, Sub, SubAssign};

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct OrdF32(f32);

impl OrdF32 {
	pub const ZERO: Self = Self(0.);

	pub fn try_new(value: f32) -> Option<Self> {
		if value.is_nan() {
			None
		} else {
			Some(Self(value))
		}
	}

	pub fn new_or_zero(value: f32) -> Self {
		Self::try_new(value).unwrap_or(Self::ZERO)
	}

	pub fn get(self) -> f32 {
		self.0
	}
}

impl Add for OrdF32 {
	type Output = Self;

	fn add(self, rhs: Self) -> Self::Output {
		Self(self.0 + rhs.0)
	}
}

impl AddAssign for OrdF32 {
	fn add_assign(&mut self, rhs: Self) {
		*self = *self + rhs;
	}
}

impl Sub for OrdF32 {
	type Output = Self;

	fn sub(self, rhs: Self) -> Self::Output {
		Self(self.0 - rhs.0)
	}
}

impl SubAssign for OrdF32 {
	fn sub_assign(&mut self, rhs: Self) {
		*self = *self - rhs;
	}
}

impl Mul for OrdF32 {
	type Output = Self;

	fn mul(self, rhs: Self) -> Self::Output {
		Self(self.0 * rhs.0)
	}
}

impl MulAssign for OrdF32 {
	fn mul_assign(&mut self, rhs: Self) {
		*self = *self * rhs;
	}
}

impl PartialOrd for OrdF32 {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		self.0.partial_cmp(&other.0)
	}
}

impl Ord for OrdF32 {
	fn cmp(&self, other: &Self) -> Ordering {
		self.partial_cmp(other).unwrap()
	}
}

impl Eq for OrdF32 {}

impl Display for OrdF32 {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		Display::fmt(&self.0, f)
	}
}

pub fn frac_to_f32(num: u32, max: u32) -> Option<f32> {
	if max != 0 {
		// Yes, there are truncation errors with this routine. However, none of the routines
		// using this object are dealing with big fractions so this is fine.
		Some((num as f64 / max as f64) as f32)
	} else {
		None
	}
}
