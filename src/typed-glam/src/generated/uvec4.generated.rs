use crate::backing_vec::Sealed;
use crate::{BackingVec, TypedVectorImpl, VecFlavor};
use core::convert::{AsMut, AsRef, From};
use core::ops::{
	Add, AddAssign, BitAnd, BitOr, BitXor, Div, DivAssign, Index, IndexMut, Mul, MulAssign, Not,
	Rem, RemAssign, Sub, SubAssign,
};
use glam::bool::BVec4;
use glam::u32::UVec4;

// === Inherent `impl` items === //

impl<M> TypedVectorImpl<UVec4, M>
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	pub const ZERO: Self = Self::from_raw(UVec4::ZERO);

	pub const ONE: Self = Self::from_raw(UVec4::ONE);

	pub const X: Self = Self::from_raw(UVec4::X);
	pub const Y: Self = Self::from_raw(UVec4::Y);
	pub const Z: Self = Self::from_raw(UVec4::Z);
	pub const W: Self = Self::from_raw(UVec4::W);

	pub const AXES: [Self; 4] = [Self::X, Self::Y, Self::Z, Self::W];

	pub const fn new(x: u32, y: u32, z: u32, w: u32) -> Self {
		Self::from_raw(UVec4::new(x, y, z, w))
	}

	pub const fn splat(v: u32) -> Self {
		Self::from_raw(UVec4::splat(v))
	}

	pub fn select(mask: BVec4, if_true: Self, if_false: Self) -> Self {
		Self::from_raw(UVec4::select(mask, if_true.into_raw(), if_false.into_raw()))
	}

	pub const fn from_array(a: [u32; 4]) -> Self {
		Self::from_raw(UVec4::from_array(a))
	}

	pub const fn to_array(&self) -> [u32; 4] {
		self.vec.to_array()
	}

	pub const fn from_slice(slice: &[u32]) -> Self {
		Self::from_raw(UVec4::from_slice(slice))
	}

	pub fn write_to_slice(self, slice: &mut [u32]) {
		self.vec.write_to_slice(slice)
	}

	pub fn dot(self, rhs: Self) -> u32 {
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

	pub fn min_element(self) -> u32 {
		self.vec.min_element()
	}

	pub fn max_element(self) -> u32 {
		self.vec.max_element()
	}

	pub fn cmpeq(self, rhs: Self) -> BVec4 {
		self.vec.cmpeq(rhs.into_raw())
	}

	pub fn cmpne(self, rhs: Self) -> BVec4 {
		self.vec.cmpne(rhs.into_raw())
	}

	pub fn cmpge(self, rhs: Self) -> BVec4 {
		self.vec.cmpge(rhs.into_raw())
	}

	pub fn cmpgt(self, rhs: Self) -> BVec4 {
		self.vec.cmpgt(rhs.into_raw())
	}

	pub fn cmple(self, rhs: Self) -> BVec4 {
		self.vec.cmple(rhs.into_raw())
	}

	pub fn cmplt(self, rhs: Self) -> BVec4 {
		self.vec.cmplt(rhs.into_raw())
	}
}

// === Misc trait derivations === //
// (most other traits are derived via trait logic in `lib.rs`)

impl BackingVec for UVec4 {}
impl Sealed for UVec4 {}

// Raw <-> Typed
impl<M> From<UVec4> for TypedVectorImpl<UVec4, M>
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	fn from(v: UVec4) -> Self {
		Self::from_raw(v)
	}
}

impl<M> From<TypedVectorImpl<UVec4, M>> for UVec4
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	fn from(v: TypedVectorImpl<UVec4, M>) -> Self {
		v.into_raw()
	}
}

// [u32; 4] <-> Typed
impl<M> From<[u32; 4]> for TypedVectorImpl<UVec4, M>
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	fn from(v: [u32; 4]) -> Self {
		UVec4::from(v).into()
	}
}

impl<M> From<TypedVectorImpl<UVec4, M>> for [u32; 4]
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	fn from(v: TypedVectorImpl<UVec4, M>) -> Self {
		v.into_raw().into()
	}
}

// (u32, ..., u32) <-> Typed
impl<M> From<(u32, u32, u32, u32)> for TypedVectorImpl<UVec4, M>
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	fn from(v: (u32, u32, u32, u32)) -> Self {
		UVec4::from(v).into()
	}
}

impl<M> From<TypedVectorImpl<UVec4, M>> for (u32, u32, u32, u32)
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	fn from(v: TypedVectorImpl<UVec4, M>) -> Self {
		v.into_raw().into()
	}
}

// `AsRef` and `AsMut`
impl<M> AsRef<[u32; 4]> for TypedVectorImpl<UVec4, M>
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	fn as_ref(&self) -> &[u32; 4] {
		self.raw().as_ref()
	}
}

impl<M> AsMut<[u32; 4]> for TypedVectorImpl<UVec4, M>
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	fn as_mut(&mut self) -> &mut [u32; 4] {
		self.raw_mut().as_mut()
	}
}

// `Index` and `IndexMut`
impl<M> Index<usize> for TypedVectorImpl<UVec4, M>
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	type Output = u32;

	fn index(&self, i: usize) -> &u32 {
		&self.raw()[i]
	}
}

impl<M> IndexMut<usize> for TypedVectorImpl<UVec4, M>
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	fn index_mut(&mut self, i: usize) -> &mut u32 {
		&mut self.raw_mut()[i]
	}
}
// === `core::ops` trait forwards === //

// `Add` operation forwarding

impl<M> Add for TypedVectorImpl<UVec4, M>
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	type Output = Self;

	fn add(self, rhs: Self) -> Self {
		self.map_raw(|lhs| Add::add(lhs, rhs.into_raw()))
	}
}

impl<M> Add<UVec4> for TypedVectorImpl<UVec4, M>
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	type Output = Self;

	fn add(self, rhs: UVec4) -> Self {
		self.map_raw(|lhs| Add::add(lhs, rhs))
	}
}

impl<M> Add<u32> for TypedVectorImpl<UVec4, M>
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	type Output = Self;

	fn add(self, rhs: u32) -> Self {
		self.map_raw(|lhs| Add::add(lhs, rhs))
	}
}

impl<M> Add<TypedVectorImpl<UVec4, M>> for u32
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	type Output = TypedVectorImpl<UVec4, M>;

	fn add(self, rhs: TypedVectorImpl<UVec4, M>) -> TypedVectorImpl<UVec4, M> {
		rhs.map_raw(|rhs| Add::add(self, rhs))
	}
}

impl<M> AddAssign for TypedVectorImpl<UVec4, M>
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	fn add_assign(&mut self, rhs: Self) {
		AddAssign::add_assign(self.raw_mut(), rhs.into_raw())
	}
}

impl<M> AddAssign<UVec4> for TypedVectorImpl<UVec4, M>
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	fn add_assign(&mut self, rhs: UVec4) {
		AddAssign::add_assign(self.raw_mut(), rhs)
	}
}

// `Sub` operation forwarding

impl<M> Sub for TypedVectorImpl<UVec4, M>
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	type Output = Self;

	fn sub(self, rhs: Self) -> Self {
		self.map_raw(|lhs| Sub::sub(lhs, rhs.into_raw()))
	}
}

impl<M> Sub<UVec4> for TypedVectorImpl<UVec4, M>
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	type Output = Self;

	fn sub(self, rhs: UVec4) -> Self {
		self.map_raw(|lhs| Sub::sub(lhs, rhs))
	}
}

impl<M> Sub<u32> for TypedVectorImpl<UVec4, M>
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	type Output = Self;

	fn sub(self, rhs: u32) -> Self {
		self.map_raw(|lhs| Sub::sub(lhs, rhs))
	}
}

impl<M> Sub<TypedVectorImpl<UVec4, M>> for u32
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	type Output = TypedVectorImpl<UVec4, M>;

	fn sub(self, rhs: TypedVectorImpl<UVec4, M>) -> TypedVectorImpl<UVec4, M> {
		rhs.map_raw(|rhs| Sub::sub(self, rhs))
	}
}

impl<M> SubAssign for TypedVectorImpl<UVec4, M>
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	fn sub_assign(&mut self, rhs: Self) {
		SubAssign::sub_assign(self.raw_mut(), rhs.into_raw())
	}
}

impl<M> SubAssign<UVec4> for TypedVectorImpl<UVec4, M>
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	fn sub_assign(&mut self, rhs: UVec4) {
		SubAssign::sub_assign(self.raw_mut(), rhs)
	}
}

// `Mul` operation forwarding

impl<M> Mul for TypedVectorImpl<UVec4, M>
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	type Output = Self;

	fn mul(self, rhs: Self) -> Self {
		self.map_raw(|lhs| Mul::mul(lhs, rhs.into_raw()))
	}
}

impl<M> Mul<UVec4> for TypedVectorImpl<UVec4, M>
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	type Output = Self;

	fn mul(self, rhs: UVec4) -> Self {
		self.map_raw(|lhs| Mul::mul(lhs, rhs))
	}
}

impl<M> Mul<u32> for TypedVectorImpl<UVec4, M>
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	type Output = Self;

	fn mul(self, rhs: u32) -> Self {
		self.map_raw(|lhs| Mul::mul(lhs, rhs))
	}
}

impl<M> Mul<TypedVectorImpl<UVec4, M>> for u32
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	type Output = TypedVectorImpl<UVec4, M>;

	fn mul(self, rhs: TypedVectorImpl<UVec4, M>) -> TypedVectorImpl<UVec4, M> {
		rhs.map_raw(|rhs| Mul::mul(self, rhs))
	}
}

impl<M> MulAssign for TypedVectorImpl<UVec4, M>
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	fn mul_assign(&mut self, rhs: Self) {
		MulAssign::mul_assign(self.raw_mut(), rhs.into_raw())
	}
}

impl<M> MulAssign<UVec4> for TypedVectorImpl<UVec4, M>
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	fn mul_assign(&mut self, rhs: UVec4) {
		MulAssign::mul_assign(self.raw_mut(), rhs)
	}
}

// `Div` operation forwarding

impl<M> Div for TypedVectorImpl<UVec4, M>
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	type Output = Self;

	fn div(self, rhs: Self) -> Self {
		self.map_raw(|lhs| Div::div(lhs, rhs.into_raw()))
	}
}

impl<M> Div<UVec4> for TypedVectorImpl<UVec4, M>
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	type Output = Self;

	fn div(self, rhs: UVec4) -> Self {
		self.map_raw(|lhs| Div::div(lhs, rhs))
	}
}

impl<M> Div<u32> for TypedVectorImpl<UVec4, M>
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	type Output = Self;

	fn div(self, rhs: u32) -> Self {
		self.map_raw(|lhs| Div::div(lhs, rhs))
	}
}

impl<M> Div<TypedVectorImpl<UVec4, M>> for u32
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	type Output = TypedVectorImpl<UVec4, M>;

	fn div(self, rhs: TypedVectorImpl<UVec4, M>) -> TypedVectorImpl<UVec4, M> {
		rhs.map_raw(|rhs| Div::div(self, rhs))
	}
}

impl<M> DivAssign for TypedVectorImpl<UVec4, M>
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	fn div_assign(&mut self, rhs: Self) {
		DivAssign::div_assign(self.raw_mut(), rhs.into_raw())
	}
}

impl<M> DivAssign<UVec4> for TypedVectorImpl<UVec4, M>
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	fn div_assign(&mut self, rhs: UVec4) {
		DivAssign::div_assign(self.raw_mut(), rhs)
	}
}

// `Rem` operation forwarding

impl<M> Rem for TypedVectorImpl<UVec4, M>
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	type Output = Self;

	fn rem(self, rhs: Self) -> Self {
		self.map_raw(|lhs| Rem::rem(lhs, rhs.into_raw()))
	}
}

impl<M> Rem<UVec4> for TypedVectorImpl<UVec4, M>
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	type Output = Self;

	fn rem(self, rhs: UVec4) -> Self {
		self.map_raw(|lhs| Rem::rem(lhs, rhs))
	}
}

impl<M> Rem<u32> for TypedVectorImpl<UVec4, M>
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	type Output = Self;

	fn rem(self, rhs: u32) -> Self {
		self.map_raw(|lhs| Rem::rem(lhs, rhs))
	}
}

impl<M> Rem<TypedVectorImpl<UVec4, M>> for u32
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	type Output = TypedVectorImpl<UVec4, M>;

	fn rem(self, rhs: TypedVectorImpl<UVec4, M>) -> TypedVectorImpl<UVec4, M> {
		rhs.map_raw(|rhs| Rem::rem(self, rhs))
	}
}

impl<M> RemAssign for TypedVectorImpl<UVec4, M>
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	fn rem_assign(&mut self, rhs: Self) {
		RemAssign::rem_assign(self.raw_mut(), rhs.into_raw())
	}
}

impl<M> RemAssign<UVec4> for TypedVectorImpl<UVec4, M>
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	fn rem_assign(&mut self, rhs: UVec4) {
		RemAssign::rem_assign(self.raw_mut(), rhs)
	}
}

// `BitAnd` operation forwarding

impl<M> BitAnd for TypedVectorImpl<UVec4, M>
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	type Output = Self;

	fn bitand(self, rhs: Self) -> Self {
		self.map_raw(|lhs| BitAnd::bitand(lhs, rhs.into_raw()))
	}
}

impl<M> BitAnd<UVec4> for TypedVectorImpl<UVec4, M>
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	type Output = Self;

	fn bitand(self, rhs: UVec4) -> Self {
		self.map_raw(|lhs| BitAnd::bitand(lhs, rhs))
	}
}

impl<M> BitAnd<u32> for TypedVectorImpl<UVec4, M>
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	type Output = Self;

	fn bitand(self, rhs: u32) -> Self {
		self.map_raw(|lhs| BitAnd::bitand(lhs, rhs))
	}
}

// `BitOr` operation forwarding

impl<M> BitOr for TypedVectorImpl<UVec4, M>
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	type Output = Self;

	fn bitor(self, rhs: Self) -> Self {
		self.map_raw(|lhs| BitOr::bitor(lhs, rhs.into_raw()))
	}
}

impl<M> BitOr<UVec4> for TypedVectorImpl<UVec4, M>
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	type Output = Self;

	fn bitor(self, rhs: UVec4) -> Self {
		self.map_raw(|lhs| BitOr::bitor(lhs, rhs))
	}
}

impl<M> BitOr<u32> for TypedVectorImpl<UVec4, M>
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	type Output = Self;

	fn bitor(self, rhs: u32) -> Self {
		self.map_raw(|lhs| BitOr::bitor(lhs, rhs))
	}
}

// `BitXor` operation forwarding

impl<M> BitXor for TypedVectorImpl<UVec4, M>
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	type Output = Self;

	fn bitxor(self, rhs: Self) -> Self {
		self.map_raw(|lhs| BitXor::bitxor(lhs, rhs.into_raw()))
	}
}

impl<M> BitXor<UVec4> for TypedVectorImpl<UVec4, M>
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	type Output = Self;

	fn bitxor(self, rhs: UVec4) -> Self {
		self.map_raw(|lhs| BitXor::bitxor(lhs, rhs))
	}
}

impl<M> BitXor<u32> for TypedVectorImpl<UVec4, M>
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	type Output = Self;

	fn bitxor(self, rhs: u32) -> Self {
		self.map_raw(|lhs| BitXor::bitxor(lhs, rhs))
	}
}

impl<M> Not for TypedVectorImpl<UVec4, M>
where
	M: ?Sized + VecFlavor<Backing = UVec4>,
{
	type Output = Self;

	fn not(self) -> Self {
		self.map_raw(|v| !v)
	}
}
