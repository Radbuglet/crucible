use crate::backing_vec::Sealed;
use crate::{BackingVec, TypedVectorImpl, VecFlavor};
use core::convert::{AsMut, AsRef, From};
use core::ops::{
	Add, AddAssign, Div, DivAssign, Index, IndexMut, Mul, MulAssign, Neg, Rem, RemAssign, Sub,
	SubAssign,
};
use glam::bool::BVec4A;
use glam::f32::Vec4;

// === Inherent `impl` items === //

impl<M> TypedVectorImpl<Vec4, M>
where
	M: ?Sized + VecFlavor<Backing = Vec4>,
{
	pub const ZERO: Self = Self::from_raw(Vec4::ZERO);

	pub const ONE: Self = Self::from_raw(Vec4::ONE);

	pub const X: Self = Self::from_raw(Vec4::X);
	pub const Y: Self = Self::from_raw(Vec4::Y);
	pub const Z: Self = Self::from_raw(Vec4::Z);
	pub const W: Self = Self::from_raw(Vec4::W);

	pub const NEG_ONE: Self = Self::from_raw(Vec4::NEG_ONE);

	pub const NEG_X: Self = Self::from_raw(Vec4::NEG_X);
	pub const NEG_Y: Self = Self::from_raw(Vec4::NEG_Y);
	pub const NEG_Z: Self = Self::from_raw(Vec4::NEG_Z);
	pub const NEG_W: Self = Self::from_raw(Vec4::NEG_W);

	pub const NAN: Self = Self::from_raw(Vec4::NAN);

	pub const AXES: [Self; 4] = [Self::X, Self::Y, Self::Z, Self::W];

	pub const fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
		Self::from_raw(Vec4::new(x, y, z, w))
	}

	pub const fn splat(v: f32) -> Self {
		Self::from_raw(Vec4::splat(v))
	}

	pub fn select(mask: BVec4A, if_true: Self, if_false: Self) -> Self {
		Self::from_raw(Vec4::select(mask, if_true.into_raw(), if_false.into_raw()))
	}

	pub const fn from_array(a: [f32; 4]) -> Self {
		Self::from_raw(Vec4::from_array(a))
	}

	pub const fn to_array(&self) -> [f32; 4] {
		self.vec.to_array()
	}

	pub const fn from_slice(slice: &[f32]) -> Self {
		Self::from_raw(Vec4::from_slice(slice))
	}

	pub fn write_to_slice(self, slice: &mut [f32]) {
		self.vec.write_to_slice(slice)
	}

	pub fn dot(self, rhs: Self) -> f32 {
		self.vec.dot(rhs.into_raw())
	}

	pub fn min(self, rhs: Self) -> Self {
		Self::from_raw(self.vec.min(rhs.into_raw()))
	}

	pub fn max(self, rhs: Self) -> Self {
		Self::from_raw(self.vec.max(rhs.into_raw()))
	}

	pub fn clamp(self, min: Self, max: Self) -> Self {
		Self::from_raw(self.vec.clamp(min.into_raw(), max.into_raw()))
	}

	pub fn min_element(self) -> f32 {
		self.vec.min_element()
	}

	pub fn max_element(self) -> f32 {
		self.vec.max_element()
	}

	pub fn cmpeq(self, rhs: Self) -> BVec4A {
		self.vec.cmpeq(rhs.into_raw())
	}

	pub fn cmpne(self, rhs: Self) -> BVec4A {
		self.vec.cmpne(rhs.into_raw())
	}

	pub fn cmpge(self, rhs: Self) -> BVec4A {
		self.vec.cmpge(rhs.into_raw())
	}

	pub fn cmpgt(self, rhs: Self) -> BVec4A {
		self.vec.cmpgt(rhs.into_raw())
	}

	pub fn cmple(self, rhs: Self) -> BVec4A {
		self.vec.cmple(rhs.into_raw())
	}

	pub fn cmplt(self, rhs: Self) -> BVec4A {
		self.vec.cmplt(rhs.into_raw())
	}

	pub fn abs(self) -> Self {
		Self::from_raw(self.vec.abs())
	}

	pub fn signum(self) -> Self {
		Self::from_raw(self.vec.signum())
	}

	pub fn is_finite(self) -> bool {
		self.vec.is_finite()
	}

	pub fn is_nan(self) -> bool {
		self.vec.is_nan()
	}

	pub fn is_nan_mask(self) -> BVec4A {
		self.vec.is_nan_mask()
	}

	pub fn length(self) -> f32 {
		self.vec.length()
	}

	pub fn length_squared(self) -> f32 {
		self.vec.length_squared()
	}

	pub fn length_recip(self) -> f32 {
		self.vec.length_recip()
	}

	pub fn distance(self, rhs: Self) -> f32 {
		self.vec.distance(rhs.into_raw())
	}

	pub fn distance_squared(self, rhs: Self) -> f32 {
		self.vec.distance_squared(rhs.into_raw())
	}

	pub fn normalize(self) -> Self {
		Self::from_raw(self.vec.normalize())
	}

	pub fn try_normalize(self) -> Option<Self> {
		self.vec.try_normalize().map(Self::from_raw)
	}

	pub fn normalize_or_zero(self) -> Self {
		Self::from_raw(self.vec.normalize_or_zero())
	}

	pub fn is_normalized(self) -> bool {
		self.vec.is_normalized()
	}

	pub fn project_onto(self, rhs: Self) -> Self {
		Self::from_raw(self.vec.project_onto(rhs.into_raw()))
	}

	pub fn reject_from(self, rhs: Self) -> Self {
		Self::from_raw(self.vec.reject_from(rhs.into_raw()))
	}

	pub fn project_onto_normalized(self, rhs: Self) -> Self {
		Self::from_raw(self.vec.project_onto_normalized(rhs.into_raw()))
	}

	pub fn reject_from_normalized(self, rhs: Self) -> Self {
		Self::from_raw(self.vec.reject_from_normalized(rhs.into_raw()))
	}

	pub fn round(self) -> Self {
		Self::from_raw(self.vec.round())
	}

	pub fn floor(self) -> Self {
		Self::from_raw(self.vec.floor())
	}

	pub fn ceil(self) -> Self {
		Self::from_raw(self.vec.ceil())
	}

	pub fn fract(self) -> Self {
		Self::from_raw(self.vec.fract())
	}

	pub fn exp(self) -> Self {
		Self::from_raw(self.vec.exp())
	}

	pub fn recip(self) -> Self {
		Self::from_raw(self.vec.recip())
	}

	pub fn powf(self, n: f32) -> Self {
		Self::from_raw(self.vec.powf(n))
	}

	pub fn lerp(self, rhs: Self, s: f32) -> Self {
		Self::from_raw(self.vec.lerp(rhs.into_raw(), s))
	}

	pub fn abs_diff_eq(self, rhs: Self, max_abs_diff: f32) -> bool {
		self.vec.abs_diff_eq(rhs.into_raw(), max_abs_diff)
	}

	pub fn clamp_length(self, min: f32, max: f32) -> Self {
		Self::from_raw(self.vec.clamp_length(min, max))
	}

	pub fn clamp_length_max(self, max: f32) -> Self {
		Self::from_raw(self.vec.clamp_length_max(max))
	}

	pub fn clamp_length_min(self, min: f32) -> Self {
		Self::from_raw(self.vec.clamp_length_min(min))
	}

	pub fn mul_add(self, a: Self, b: Self) -> Self {
		Self::from_raw(self.vec.mul_add(a.into_raw(), b.into_raw()))
	}
}

// === Misc trait derivations === //
// (most other traits are derived via trait logic in `lib.rs`)

impl BackingVec for Vec4 {}
impl Sealed for Vec4 {}

// Raw <-> Typed
impl<M> From<Vec4> for TypedVectorImpl<Vec4, M>
where
	M: ?Sized + VecFlavor<Backing = Vec4>,
{
	fn from(v: Vec4) -> Self {
		Self::from_raw(v)
	}
}

impl<M> From<TypedVectorImpl<Vec4, M>> for Vec4
where
	M: ?Sized + VecFlavor<Backing = Vec4>,
{
	fn from(v: TypedVectorImpl<Vec4, M>) -> Self {
		v.into_raw()
	}
}

// [f32; 4] <-> Typed
impl<M> From<[f32; 4]> for TypedVectorImpl<Vec4, M>
where
	M: ?Sized + VecFlavor<Backing = Vec4>,
{
	fn from(v: [f32; 4]) -> Self {
		Vec4::from(v).into()
	}
}

impl<M> From<TypedVectorImpl<Vec4, M>> for [f32; 4]
where
	M: ?Sized + VecFlavor<Backing = Vec4>,
{
	fn from(v: TypedVectorImpl<Vec4, M>) -> Self {
		v.into_raw().into()
	}
}

// (f32, ..., f32) <-> Typed
impl<M> From<(f32, f32, f32, f32)> for TypedVectorImpl<Vec4, M>
where
	M: ?Sized + VecFlavor<Backing = Vec4>,
{
	fn from(v: (f32, f32, f32, f32)) -> Self {
		Vec4::from(v).into()
	}
}

impl<M> From<TypedVectorImpl<Vec4, M>> for (f32, f32, f32, f32)
where
	M: ?Sized + VecFlavor<Backing = Vec4>,
{
	fn from(v: TypedVectorImpl<Vec4, M>) -> Self {
		v.into_raw().into()
	}
}

// `AsRef` and `AsMut`
impl<M> AsRef<[f32; 4]> for TypedVectorImpl<Vec4, M>
where
	M: ?Sized + VecFlavor<Backing = Vec4>,
{
	fn as_ref(&self) -> &[f32; 4] {
		self.raw().as_ref()
	}
}

impl<M> AsMut<[f32; 4]> for TypedVectorImpl<Vec4, M>
where
	M: ?Sized + VecFlavor<Backing = Vec4>,
{
	fn as_mut(&mut self) -> &mut [f32; 4] {
		self.raw_mut().as_mut()
	}
}

// `Index` and `IndexMut`
impl<M> Index<usize> for TypedVectorImpl<Vec4, M>
where
	M: ?Sized + VecFlavor<Backing = Vec4>,
{
	type Output = f32;

	fn index(&self, i: usize) -> &f32 {
		&self.raw()[i]
	}
}

impl<M> IndexMut<usize> for TypedVectorImpl<Vec4, M>
where
	M: ?Sized + VecFlavor<Backing = Vec4>,
{
	fn index_mut(&mut self, i: usize) -> &mut f32 {
		&mut self.raw_mut()[i]
	}
}
// === `core::ops` trait forwards === //

// `Add` operation forwarding

impl<M> Add for TypedVectorImpl<Vec4, M>
where
	M: ?Sized + VecFlavor<Backing = Vec4>,
{
	type Output = Self;

	fn add(self, rhs: Self) -> Self {
		self.map_raw(|lhs| Add::add(lhs, rhs.into_raw()))
	}
}

impl<M> Add<Vec4> for TypedVectorImpl<Vec4, M>
where
	M: ?Sized + VecFlavor<Backing = Vec4>,
{
	type Output = Self;

	fn add(self, rhs: Vec4) -> Self {
		self.map_raw(|lhs| Add::add(lhs, rhs))
	}
}

impl<M> Add<f32> for TypedVectorImpl<Vec4, M>
where
	M: ?Sized + VecFlavor<Backing = Vec4>,
{
	type Output = Self;

	fn add(self, rhs: f32) -> Self {
		self.map_raw(|lhs| Add::add(lhs, rhs))
	}
}

impl<M> Add<TypedVectorImpl<Vec4, M>> for f32
where
	M: ?Sized + VecFlavor<Backing = Vec4>,
{
	type Output = TypedVectorImpl<Vec4, M>;

	fn add(self, rhs: TypedVectorImpl<Vec4, M>) -> TypedVectorImpl<Vec4, M> {
		rhs.map_raw(|rhs| Add::add(self, rhs))
	}
}

impl<M> AddAssign for TypedVectorImpl<Vec4, M>
where
	M: ?Sized + VecFlavor<Backing = Vec4>,
{
	fn add_assign(&mut self, rhs: Self) {
		AddAssign::add_assign(self.raw_mut(), rhs.into_raw())
	}
}

impl<M> AddAssign<Vec4> for TypedVectorImpl<Vec4, M>
where
	M: ?Sized + VecFlavor<Backing = Vec4>,
{
	fn add_assign(&mut self, rhs: Vec4) {
		AddAssign::add_assign(self.raw_mut(), rhs)
	}
}

// `Sub` operation forwarding

impl<M> Sub for TypedVectorImpl<Vec4, M>
where
	M: ?Sized + VecFlavor<Backing = Vec4>,
{
	type Output = Self;

	fn sub(self, rhs: Self) -> Self {
		self.map_raw(|lhs| Sub::sub(lhs, rhs.into_raw()))
	}
}

impl<M> Sub<Vec4> for TypedVectorImpl<Vec4, M>
where
	M: ?Sized + VecFlavor<Backing = Vec4>,
{
	type Output = Self;

	fn sub(self, rhs: Vec4) -> Self {
		self.map_raw(|lhs| Sub::sub(lhs, rhs))
	}
}

impl<M> Sub<f32> for TypedVectorImpl<Vec4, M>
where
	M: ?Sized + VecFlavor<Backing = Vec4>,
{
	type Output = Self;

	fn sub(self, rhs: f32) -> Self {
		self.map_raw(|lhs| Sub::sub(lhs, rhs))
	}
}

impl<M> Sub<TypedVectorImpl<Vec4, M>> for f32
where
	M: ?Sized + VecFlavor<Backing = Vec4>,
{
	type Output = TypedVectorImpl<Vec4, M>;

	fn sub(self, rhs: TypedVectorImpl<Vec4, M>) -> TypedVectorImpl<Vec4, M> {
		rhs.map_raw(|rhs| Sub::sub(self, rhs))
	}
}

impl<M> SubAssign for TypedVectorImpl<Vec4, M>
where
	M: ?Sized + VecFlavor<Backing = Vec4>,
{
	fn sub_assign(&mut self, rhs: Self) {
		SubAssign::sub_assign(self.raw_mut(), rhs.into_raw())
	}
}

impl<M> SubAssign<Vec4> for TypedVectorImpl<Vec4, M>
where
	M: ?Sized + VecFlavor<Backing = Vec4>,
{
	fn sub_assign(&mut self, rhs: Vec4) {
		SubAssign::sub_assign(self.raw_mut(), rhs)
	}
}

// `Mul` operation forwarding

impl<M> Mul for TypedVectorImpl<Vec4, M>
where
	M: ?Sized + VecFlavor<Backing = Vec4>,
{
	type Output = Self;

	fn mul(self, rhs: Self) -> Self {
		self.map_raw(|lhs| Mul::mul(lhs, rhs.into_raw()))
	}
}

impl<M> Mul<Vec4> for TypedVectorImpl<Vec4, M>
where
	M: ?Sized + VecFlavor<Backing = Vec4>,
{
	type Output = Self;

	fn mul(self, rhs: Vec4) -> Self {
		self.map_raw(|lhs| Mul::mul(lhs, rhs))
	}
}

impl<M> Mul<f32> for TypedVectorImpl<Vec4, M>
where
	M: ?Sized + VecFlavor<Backing = Vec4>,
{
	type Output = Self;

	fn mul(self, rhs: f32) -> Self {
		self.map_raw(|lhs| Mul::mul(lhs, rhs))
	}
}

impl<M> Mul<TypedVectorImpl<Vec4, M>> for f32
where
	M: ?Sized + VecFlavor<Backing = Vec4>,
{
	type Output = TypedVectorImpl<Vec4, M>;

	fn mul(self, rhs: TypedVectorImpl<Vec4, M>) -> TypedVectorImpl<Vec4, M> {
		rhs.map_raw(|rhs| Mul::mul(self, rhs))
	}
}

impl<M> MulAssign for TypedVectorImpl<Vec4, M>
where
	M: ?Sized + VecFlavor<Backing = Vec4>,
{
	fn mul_assign(&mut self, rhs: Self) {
		MulAssign::mul_assign(self.raw_mut(), rhs.into_raw())
	}
}

impl<M> MulAssign<Vec4> for TypedVectorImpl<Vec4, M>
where
	M: ?Sized + VecFlavor<Backing = Vec4>,
{
	fn mul_assign(&mut self, rhs: Vec4) {
		MulAssign::mul_assign(self.raw_mut(), rhs)
	}
}

// `Div` operation forwarding

impl<M> Div for TypedVectorImpl<Vec4, M>
where
	M: ?Sized + VecFlavor<Backing = Vec4>,
{
	type Output = Self;

	fn div(self, rhs: Self) -> Self {
		self.map_raw(|lhs| Div::div(lhs, rhs.into_raw()))
	}
}

impl<M> Div<Vec4> for TypedVectorImpl<Vec4, M>
where
	M: ?Sized + VecFlavor<Backing = Vec4>,
{
	type Output = Self;

	fn div(self, rhs: Vec4) -> Self {
		self.map_raw(|lhs| Div::div(lhs, rhs))
	}
}

impl<M> Div<f32> for TypedVectorImpl<Vec4, M>
where
	M: ?Sized + VecFlavor<Backing = Vec4>,
{
	type Output = Self;

	fn div(self, rhs: f32) -> Self {
		self.map_raw(|lhs| Div::div(lhs, rhs))
	}
}

impl<M> Div<TypedVectorImpl<Vec4, M>> for f32
where
	M: ?Sized + VecFlavor<Backing = Vec4>,
{
	type Output = TypedVectorImpl<Vec4, M>;

	fn div(self, rhs: TypedVectorImpl<Vec4, M>) -> TypedVectorImpl<Vec4, M> {
		rhs.map_raw(|rhs| Div::div(self, rhs))
	}
}

impl<M> DivAssign for TypedVectorImpl<Vec4, M>
where
	M: ?Sized + VecFlavor<Backing = Vec4>,
{
	fn div_assign(&mut self, rhs: Self) {
		DivAssign::div_assign(self.raw_mut(), rhs.into_raw())
	}
}

impl<M> DivAssign<Vec4> for TypedVectorImpl<Vec4, M>
where
	M: ?Sized + VecFlavor<Backing = Vec4>,
{
	fn div_assign(&mut self, rhs: Vec4) {
		DivAssign::div_assign(self.raw_mut(), rhs)
	}
}

// `Rem` operation forwarding

impl<M> Rem for TypedVectorImpl<Vec4, M>
where
	M: ?Sized + VecFlavor<Backing = Vec4>,
{
	type Output = Self;

	fn rem(self, rhs: Self) -> Self {
		self.map_raw(|lhs| Rem::rem(lhs, rhs.into_raw()))
	}
}

impl<M> Rem<Vec4> for TypedVectorImpl<Vec4, M>
where
	M: ?Sized + VecFlavor<Backing = Vec4>,
{
	type Output = Self;

	fn rem(self, rhs: Vec4) -> Self {
		self.map_raw(|lhs| Rem::rem(lhs, rhs))
	}
}

impl<M> Rem<f32> for TypedVectorImpl<Vec4, M>
where
	M: ?Sized + VecFlavor<Backing = Vec4>,
{
	type Output = Self;

	fn rem(self, rhs: f32) -> Self {
		self.map_raw(|lhs| Rem::rem(lhs, rhs))
	}
}

impl<M> Rem<TypedVectorImpl<Vec4, M>> for f32
where
	M: ?Sized + VecFlavor<Backing = Vec4>,
{
	type Output = TypedVectorImpl<Vec4, M>;

	fn rem(self, rhs: TypedVectorImpl<Vec4, M>) -> TypedVectorImpl<Vec4, M> {
		rhs.map_raw(|rhs| Rem::rem(self, rhs))
	}
}

impl<M> RemAssign for TypedVectorImpl<Vec4, M>
where
	M: ?Sized + VecFlavor<Backing = Vec4>,
{
	fn rem_assign(&mut self, rhs: Self) {
		RemAssign::rem_assign(self.raw_mut(), rhs.into_raw())
	}
}

impl<M> RemAssign<Vec4> for TypedVectorImpl<Vec4, M>
where
	M: ?Sized + VecFlavor<Backing = Vec4>,
{
	fn rem_assign(&mut self, rhs: Vec4) {
		RemAssign::rem_assign(self.raw_mut(), rhs)
	}
}

impl<M> Neg for TypedVectorImpl<Vec4, M>
where
	M: ?Sized + VecFlavor<Backing = Vec4>,
{
	type Output = Self;

	fn neg(self) -> Self {
		self.map_raw(|v| -v)
	}
}
