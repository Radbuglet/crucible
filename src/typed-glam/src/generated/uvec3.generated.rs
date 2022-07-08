use crate::backing_vec::Sealed;
use crate::{BackingVec, TypedVectorImpl, VecFlavor};
use core::convert::{AsMut, AsRef, From};
use core::ops::{
	Add, AddAssign, BitAnd, BitOr, BitXor, Div, DivAssign, Index, IndexMut, Mul, MulAssign, Not,
	Rem, RemAssign, Sub, SubAssign,
};
use glam::bool::BVec3;
use glam::u32::UVec3;

// === Inherent `impl` items === //

impl<M> TypedVectorImpl<UVec3, M>
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	pub const ZERO: Self = Self::from_raw(UVec3::ZERO);

	pub const ONE: Self = Self::from_raw(UVec3::ONE);

	pub const X: Self = Self::from_raw(UVec3::X);
	pub const Y: Self = Self::from_raw(UVec3::Y);
	pub const Z: Self = Self::from_raw(UVec3::Z);

	pub const AXES: [Self; 3] = [Self::X, Self::Y, Self::Z];

	pub const fn new(x: u32, y: u32, z: u32) -> Self {
		Self::from_raw(UVec3::new(x, y, z))
	}

	pub const fn splat(v: u32) -> Self {
		Self::from_raw(UVec3::splat(v))
	}

	pub fn select(mask: BVec3, if_true: Self, if_false: Self) -> Self {
		Self::from_raw(UVec3::select(mask, if_true.into_raw(), if_false.into_raw()))
	}

	pub const fn from_array(a: [u32; 3]) -> Self {
		Self::from_raw(UVec3::from_array(a))
	}

	pub const fn to_array(&self) -> [u32; 3] {
		self.vec.to_array()
	}

	pub const fn from_slice(slice: &[u32]) -> Self {
		Self::from_raw(UVec3::from_slice(slice))
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

	pub fn cmpeq(self, rhs: Self) -> BVec3 {
		self.vec.cmpeq(rhs.into_raw())
	}

	pub fn cmpne(self, rhs: Self) -> BVec3 {
		self.vec.cmpne(rhs.into_raw())
	}

	pub fn cmpge(self, rhs: Self) -> BVec3 {
		self.vec.cmpge(rhs.into_raw())
	}

	pub fn cmpgt(self, rhs: Self) -> BVec3 {
		self.vec.cmpgt(rhs.into_raw())
	}

	pub fn cmple(self, rhs: Self) -> BVec3 {
		self.vec.cmple(rhs.into_raw())
	}

	pub fn cmplt(self, rhs: Self) -> BVec3 {
		self.vec.cmplt(rhs.into_raw())
	}
}

// === Misc trait derivations === //
// (most other traits are derived via trait logic in `lib.rs`)

impl BackingVec for UVec3 {}
impl Sealed for UVec3 {}

// Raw <-> Typed
impl<M> From<UVec3> for TypedVectorImpl<UVec3, M>
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	fn from(v: UVec3) -> Self {
		Self::from_raw(v)
	}
}

impl<M> From<TypedVectorImpl<UVec3, M>> for UVec3
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	fn from(v: TypedVectorImpl<UVec3, M>) -> Self {
		v.into_raw()
	}
}

// [u32; 3] <-> Typed
impl<M> From<[u32; 3]> for TypedVectorImpl<UVec3, M>
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	fn from(v: [u32; 3]) -> Self {
		UVec3::from(v).into()
	}
}

impl<M> From<TypedVectorImpl<UVec3, M>> for [u32; 3]
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	fn from(v: TypedVectorImpl<UVec3, M>) -> Self {
		v.into_raw().into()
	}
}

// (u32, ..., u32) <-> Typed
impl<M> From<(u32, u32, u32)> for TypedVectorImpl<UVec3, M>
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	fn from(v: (u32, u32, u32)) -> Self {
		UVec3::from(v).into()
	}
}

impl<M> From<TypedVectorImpl<UVec3, M>> for (u32, u32, u32)
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	fn from(v: TypedVectorImpl<UVec3, M>) -> Self {
		v.into_raw().into()
	}
}

// `AsRef` and `AsMut`
impl<M> AsRef<[u32; 3]> for TypedVectorImpl<UVec3, M>
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	fn as_ref(&self) -> &[u32; 3] {
		self.raw().as_ref()
	}
}

impl<M> AsMut<[u32; 3]> for TypedVectorImpl<UVec3, M>
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	fn as_mut(&mut self) -> &mut [u32; 3] {
		self.raw_mut().as_mut()
	}
}

// `Index` and `IndexMut`
impl<M> Index<usize> for TypedVectorImpl<UVec3, M>
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	type Output = u32;

	fn index(&self, i: usize) -> &u32 {
		&self.raw()[i]
	}
}

impl<M> IndexMut<usize> for TypedVectorImpl<UVec3, M>
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	fn index_mut(&mut self, i: usize) -> &mut u32 {
		&mut self.raw_mut()[i]
	}
}
// === `core::ops` trait forwards === //

// `Add` operation forwarding

impl<M> Add for TypedVectorImpl<UVec3, M>
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	type Output = Self;

	fn add(self, rhs: Self) -> Self {
		self.map_raw(|lhs| Add::add(lhs, rhs.into_raw()))
	}
}

impl<M> Add<UVec3> for TypedVectorImpl<UVec3, M>
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	type Output = Self;

	fn add(self, rhs: UVec3) -> Self {
		self.map_raw(|lhs| Add::add(lhs, rhs))
	}
}

impl<M> Add<u32> for TypedVectorImpl<UVec3, M>
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	type Output = Self;

	fn add(self, rhs: u32) -> Self {
		self.map_raw(|lhs| Add::add(lhs, rhs))
	}
}

impl<M> Add<TypedVectorImpl<UVec3, M>> for u32
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	type Output = TypedVectorImpl<UVec3, M>;

	fn add(self, rhs: TypedVectorImpl<UVec3, M>) -> TypedVectorImpl<UVec3, M> {
		rhs.map_raw(|rhs| Add::add(self, rhs))
	}
}

impl<M> AddAssign for TypedVectorImpl<UVec3, M>
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	fn add_assign(&mut self, rhs: Self) {
		AddAssign::add_assign(self.raw_mut(), rhs.into_raw())
	}
}

impl<M> AddAssign<UVec3> for TypedVectorImpl<UVec3, M>
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	fn add_assign(&mut self, rhs: UVec3) {
		AddAssign::add_assign(self.raw_mut(), rhs)
	}
}

// `Sub` operation forwarding

impl<M> Sub for TypedVectorImpl<UVec3, M>
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	type Output = Self;

	fn sub(self, rhs: Self) -> Self {
		self.map_raw(|lhs| Sub::sub(lhs, rhs.into_raw()))
	}
}

impl<M> Sub<UVec3> for TypedVectorImpl<UVec3, M>
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	type Output = Self;

	fn sub(self, rhs: UVec3) -> Self {
		self.map_raw(|lhs| Sub::sub(lhs, rhs))
	}
}

impl<M> Sub<u32> for TypedVectorImpl<UVec3, M>
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	type Output = Self;

	fn sub(self, rhs: u32) -> Self {
		self.map_raw(|lhs| Sub::sub(lhs, rhs))
	}
}

impl<M> Sub<TypedVectorImpl<UVec3, M>> for u32
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	type Output = TypedVectorImpl<UVec3, M>;

	fn sub(self, rhs: TypedVectorImpl<UVec3, M>) -> TypedVectorImpl<UVec3, M> {
		rhs.map_raw(|rhs| Sub::sub(self, rhs))
	}
}

impl<M> SubAssign for TypedVectorImpl<UVec3, M>
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	fn sub_assign(&mut self, rhs: Self) {
		SubAssign::sub_assign(self.raw_mut(), rhs.into_raw())
	}
}

impl<M> SubAssign<UVec3> for TypedVectorImpl<UVec3, M>
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	fn sub_assign(&mut self, rhs: UVec3) {
		SubAssign::sub_assign(self.raw_mut(), rhs)
	}
}

// `Mul` operation forwarding

impl<M> Mul for TypedVectorImpl<UVec3, M>
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	type Output = Self;

	fn mul(self, rhs: Self) -> Self {
		self.map_raw(|lhs| Mul::mul(lhs, rhs.into_raw()))
	}
}

impl<M> Mul<UVec3> for TypedVectorImpl<UVec3, M>
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	type Output = Self;

	fn mul(self, rhs: UVec3) -> Self {
		self.map_raw(|lhs| Mul::mul(lhs, rhs))
	}
}

impl<M> Mul<u32> for TypedVectorImpl<UVec3, M>
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	type Output = Self;

	fn mul(self, rhs: u32) -> Self {
		self.map_raw(|lhs| Mul::mul(lhs, rhs))
	}
}

impl<M> Mul<TypedVectorImpl<UVec3, M>> for u32
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	type Output = TypedVectorImpl<UVec3, M>;

	fn mul(self, rhs: TypedVectorImpl<UVec3, M>) -> TypedVectorImpl<UVec3, M> {
		rhs.map_raw(|rhs| Mul::mul(self, rhs))
	}
}

impl<M> MulAssign for TypedVectorImpl<UVec3, M>
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	fn mul_assign(&mut self, rhs: Self) {
		MulAssign::mul_assign(self.raw_mut(), rhs.into_raw())
	}
}

impl<M> MulAssign<UVec3> for TypedVectorImpl<UVec3, M>
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	fn mul_assign(&mut self, rhs: UVec3) {
		MulAssign::mul_assign(self.raw_mut(), rhs)
	}
}

// `Div` operation forwarding

impl<M> Div for TypedVectorImpl<UVec3, M>
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	type Output = Self;

	fn div(self, rhs: Self) -> Self {
		self.map_raw(|lhs| Div::div(lhs, rhs.into_raw()))
	}
}

impl<M> Div<UVec3> for TypedVectorImpl<UVec3, M>
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	type Output = Self;

	fn div(self, rhs: UVec3) -> Self {
		self.map_raw(|lhs| Div::div(lhs, rhs))
	}
}

impl<M> Div<u32> for TypedVectorImpl<UVec3, M>
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	type Output = Self;

	fn div(self, rhs: u32) -> Self {
		self.map_raw(|lhs| Div::div(lhs, rhs))
	}
}

impl<M> Div<TypedVectorImpl<UVec3, M>> for u32
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	type Output = TypedVectorImpl<UVec3, M>;

	fn div(self, rhs: TypedVectorImpl<UVec3, M>) -> TypedVectorImpl<UVec3, M> {
		rhs.map_raw(|rhs| Div::div(self, rhs))
	}
}

impl<M> DivAssign for TypedVectorImpl<UVec3, M>
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	fn div_assign(&mut self, rhs: Self) {
		DivAssign::div_assign(self.raw_mut(), rhs.into_raw())
	}
}

impl<M> DivAssign<UVec3> for TypedVectorImpl<UVec3, M>
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	fn div_assign(&mut self, rhs: UVec3) {
		DivAssign::div_assign(self.raw_mut(), rhs)
	}
}

// `Rem` operation forwarding

impl<M> Rem for TypedVectorImpl<UVec3, M>
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	type Output = Self;

	fn rem(self, rhs: Self) -> Self {
		self.map_raw(|lhs| Rem::rem(lhs, rhs.into_raw()))
	}
}

impl<M> Rem<UVec3> for TypedVectorImpl<UVec3, M>
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	type Output = Self;

	fn rem(self, rhs: UVec3) -> Self {
		self.map_raw(|lhs| Rem::rem(lhs, rhs))
	}
}

impl<M> Rem<u32> for TypedVectorImpl<UVec3, M>
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	type Output = Self;

	fn rem(self, rhs: u32) -> Self {
		self.map_raw(|lhs| Rem::rem(lhs, rhs))
	}
}

impl<M> Rem<TypedVectorImpl<UVec3, M>> for u32
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	type Output = TypedVectorImpl<UVec3, M>;

	fn rem(self, rhs: TypedVectorImpl<UVec3, M>) -> TypedVectorImpl<UVec3, M> {
		rhs.map_raw(|rhs| Rem::rem(self, rhs))
	}
}

impl<M> RemAssign for TypedVectorImpl<UVec3, M>
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	fn rem_assign(&mut self, rhs: Self) {
		RemAssign::rem_assign(self.raw_mut(), rhs.into_raw())
	}
}

impl<M> RemAssign<UVec3> for TypedVectorImpl<UVec3, M>
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	fn rem_assign(&mut self, rhs: UVec3) {
		RemAssign::rem_assign(self.raw_mut(), rhs)
	}
}

// `BitAnd` operation forwarding

impl<M> BitAnd for TypedVectorImpl<UVec3, M>
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	type Output = Self;

	fn bitand(self, rhs: Self) -> Self {
		self.map_raw(|lhs| BitAnd::bitand(lhs, rhs.into_raw()))
	}
}

impl<M> BitAnd<UVec3> for TypedVectorImpl<UVec3, M>
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	type Output = Self;

	fn bitand(self, rhs: UVec3) -> Self {
		self.map_raw(|lhs| BitAnd::bitand(lhs, rhs))
	}
}

impl<M> BitAnd<u32> for TypedVectorImpl<UVec3, M>
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	type Output = Self;

	fn bitand(self, rhs: u32) -> Self {
		self.map_raw(|lhs| BitAnd::bitand(lhs, rhs))
	}
}

// `BitOr` operation forwarding

impl<M> BitOr for TypedVectorImpl<UVec3, M>
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	type Output = Self;

	fn bitor(self, rhs: Self) -> Self {
		self.map_raw(|lhs| BitOr::bitor(lhs, rhs.into_raw()))
	}
}

impl<M> BitOr<UVec3> for TypedVectorImpl<UVec3, M>
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	type Output = Self;

	fn bitor(self, rhs: UVec3) -> Self {
		self.map_raw(|lhs| BitOr::bitor(lhs, rhs))
	}
}

impl<M> BitOr<u32> for TypedVectorImpl<UVec3, M>
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	type Output = Self;

	fn bitor(self, rhs: u32) -> Self {
		self.map_raw(|lhs| BitOr::bitor(lhs, rhs))
	}
}

// `BitXor` operation forwarding

impl<M> BitXor for TypedVectorImpl<UVec3, M>
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	type Output = Self;

	fn bitxor(self, rhs: Self) -> Self {
		self.map_raw(|lhs| BitXor::bitxor(lhs, rhs.into_raw()))
	}
}

impl<M> BitXor<UVec3> for TypedVectorImpl<UVec3, M>
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	type Output = Self;

	fn bitxor(self, rhs: UVec3) -> Self {
		self.map_raw(|lhs| BitXor::bitxor(lhs, rhs))
	}
}

impl<M> BitXor<u32> for TypedVectorImpl<UVec3, M>
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	type Output = Self;

	fn bitxor(self, rhs: u32) -> Self {
		self.map_raw(|lhs| BitXor::bitxor(lhs, rhs))
	}
}

impl<M> Not for TypedVectorImpl<UVec3, M>
where
	M: ?Sized + VecFlavor<Backing = UVec3>,
{
	type Output = Self;

	fn not(self) -> Self {
		self.map_raw(|v| !v)
	}
}
