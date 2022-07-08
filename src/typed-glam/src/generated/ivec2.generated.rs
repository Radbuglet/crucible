use crate::backing_vec::Sealed;
use crate::{BackingVec, TypedVectorImpl, VecFlavor};
use core::convert::{AsMut, AsRef, From};
use core::ops::{
	Add, AddAssign, BitAnd, BitOr, BitXor, Div, DivAssign, Index, IndexMut, Mul, MulAssign, Neg,
	Not, Rem, RemAssign, Sub, SubAssign,
};
use glam::bool::BVec2;
use glam::i32::IVec2;

// === Inherent `impl` items === //

impl<M> TypedVectorImpl<IVec2, M>
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	pub const ZERO: Self = Self::from_raw(IVec2::ZERO);

	pub const ONE: Self = Self::from_raw(IVec2::ONE);

	pub const X: Self = Self::from_raw(IVec2::X);
	pub const Y: Self = Self::from_raw(IVec2::Y);

	pub const NEG_ONE: Self = Self::from_raw(IVec2::NEG_ONE);

	pub const NEG_X: Self = Self::from_raw(IVec2::NEG_X);
	pub const NEG_Y: Self = Self::from_raw(IVec2::NEG_Y);

	pub const AXES: [Self; 2] = [Self::X, Self::Y];

	pub const fn new(x: i32, y: i32) -> Self {
		Self::from_raw(IVec2::new(x, y))
	}

	pub const fn splat(v: i32) -> Self {
		Self::from_raw(IVec2::splat(v))
	}

	pub fn select(mask: BVec2, if_true: Self, if_false: Self) -> Self {
		Self::from_raw(IVec2::select(mask, if_true.into_raw(), if_false.into_raw()))
	}

	pub const fn from_array(a: [i32; 2]) -> Self {
		Self::from_raw(IVec2::from_array(a))
	}

	pub const fn to_array(&self) -> [i32; 2] {
		self.vec.to_array()
	}

	pub const fn from_slice(slice: &[i32]) -> Self {
		Self::from_raw(IVec2::from_slice(slice))
	}

	pub fn write_to_slice(self, slice: &mut [i32]) {
		self.vec.write_to_slice(slice)
	}

	pub fn dot(self, rhs: Self) -> i32 {
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

	pub fn min_element(self) -> i32 {
		self.vec.min_element()
	}

	pub fn max_element(self) -> i32 {
		self.vec.max_element()
	}

	pub fn cmpeq(self, rhs: Self) -> BVec2 {
		self.vec.cmpeq(rhs.into_raw())
	}

	pub fn cmpne(self, rhs: Self) -> BVec2 {
		self.vec.cmpne(rhs.into_raw())
	}

	pub fn cmpge(self, rhs: Self) -> BVec2 {
		self.vec.cmpge(rhs.into_raw())
	}

	pub fn cmpgt(self, rhs: Self) -> BVec2 {
		self.vec.cmpgt(rhs.into_raw())
	}

	pub fn cmple(self, rhs: Self) -> BVec2 {
		self.vec.cmple(rhs.into_raw())
	}

	pub fn cmplt(self, rhs: Self) -> BVec2 {
		self.vec.cmplt(rhs.into_raw())
	}

	pub fn abs(self) -> Self {
		Self::from_raw(self.vec.abs())
	}

	pub fn signum(self) -> Self {
		Self::from_raw(self.vec.signum())
	}
}

// === Misc trait derivations === //
// (most other traits are derived via trait logic in `lib.rs`)

impl BackingVec for IVec2 {}
impl Sealed for IVec2 {}

// Raw <-> Typed
impl<M> From<IVec2> for TypedVectorImpl<IVec2, M>
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	fn from(v: IVec2) -> Self {
		Self::from_raw(v)
	}
}

impl<M> From<TypedVectorImpl<IVec2, M>> for IVec2
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	fn from(v: TypedVectorImpl<IVec2, M>) -> Self {
		v.into_raw()
	}
}

// [i32; 2] <-> Typed
impl<M> From<[i32; 2]> for TypedVectorImpl<IVec2, M>
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	fn from(v: [i32; 2]) -> Self {
		IVec2::from(v).into()
	}
}

impl<M> From<TypedVectorImpl<IVec2, M>> for [i32; 2]
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	fn from(v: TypedVectorImpl<IVec2, M>) -> Self {
		v.into_raw().into()
	}
}

// (i32, ..., i32) <-> Typed
impl<M> From<(i32, i32)> for TypedVectorImpl<IVec2, M>
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	fn from(v: (i32, i32)) -> Self {
		IVec2::from(v).into()
	}
}

impl<M> From<TypedVectorImpl<IVec2, M>> for (i32, i32)
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	fn from(v: TypedVectorImpl<IVec2, M>) -> Self {
		v.into_raw().into()
	}
}

// `AsRef` and `AsMut`
impl<M> AsRef<[i32; 2]> for TypedVectorImpl<IVec2, M>
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	fn as_ref(&self) -> &[i32; 2] {
		self.raw().as_ref()
	}
}

impl<M> AsMut<[i32; 2]> for TypedVectorImpl<IVec2, M>
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	fn as_mut(&mut self) -> &mut [i32; 2] {
		self.raw_mut().as_mut()
	}
}

// `Index` and `IndexMut`
impl<M> Index<usize> for TypedVectorImpl<IVec2, M>
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	type Output = i32;

	fn index(&self, i: usize) -> &i32 {
		&self.raw()[i]
	}
}

impl<M> IndexMut<usize> for TypedVectorImpl<IVec2, M>
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	fn index_mut(&mut self, i: usize) -> &mut i32 {
		&mut self.raw_mut()[i]
	}
}
// === `core::ops` trait forwards === //

// `Add` operation forwarding

impl<M> Add for TypedVectorImpl<IVec2, M>
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	type Output = Self;

	fn add(self, rhs: Self) -> Self {
		self.map_raw(|lhs| Add::add(lhs, rhs.into_raw()))
	}
}

impl<M> Add<IVec2> for TypedVectorImpl<IVec2, M>
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	type Output = Self;

	fn add(self, rhs: IVec2) -> Self {
		self.map_raw(|lhs| Add::add(lhs, rhs))
	}
}

impl<M> Add<i32> for TypedVectorImpl<IVec2, M>
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	type Output = Self;

	fn add(self, rhs: i32) -> Self {
		self.map_raw(|lhs| Add::add(lhs, rhs))
	}
}

impl<M> Add<TypedVectorImpl<IVec2, M>> for i32
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	type Output = TypedVectorImpl<IVec2, M>;

	fn add(self, rhs: TypedVectorImpl<IVec2, M>) -> TypedVectorImpl<IVec2, M> {
		rhs.map_raw(|rhs| Add::add(self, rhs))
	}
}

impl<M> AddAssign for TypedVectorImpl<IVec2, M>
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	fn add_assign(&mut self, rhs: Self) {
		AddAssign::add_assign(self.raw_mut(), rhs.into_raw())
	}
}

impl<M> AddAssign<IVec2> for TypedVectorImpl<IVec2, M>
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	fn add_assign(&mut self, rhs: IVec2) {
		AddAssign::add_assign(self.raw_mut(), rhs)
	}
}

// `Sub` operation forwarding

impl<M> Sub for TypedVectorImpl<IVec2, M>
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	type Output = Self;

	fn sub(self, rhs: Self) -> Self {
		self.map_raw(|lhs| Sub::sub(lhs, rhs.into_raw()))
	}
}

impl<M> Sub<IVec2> for TypedVectorImpl<IVec2, M>
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	type Output = Self;

	fn sub(self, rhs: IVec2) -> Self {
		self.map_raw(|lhs| Sub::sub(lhs, rhs))
	}
}

impl<M> Sub<i32> for TypedVectorImpl<IVec2, M>
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	type Output = Self;

	fn sub(self, rhs: i32) -> Self {
		self.map_raw(|lhs| Sub::sub(lhs, rhs))
	}
}

impl<M> Sub<TypedVectorImpl<IVec2, M>> for i32
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	type Output = TypedVectorImpl<IVec2, M>;

	fn sub(self, rhs: TypedVectorImpl<IVec2, M>) -> TypedVectorImpl<IVec2, M> {
		rhs.map_raw(|rhs| Sub::sub(self, rhs))
	}
}

impl<M> SubAssign for TypedVectorImpl<IVec2, M>
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	fn sub_assign(&mut self, rhs: Self) {
		SubAssign::sub_assign(self.raw_mut(), rhs.into_raw())
	}
}

impl<M> SubAssign<IVec2> for TypedVectorImpl<IVec2, M>
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	fn sub_assign(&mut self, rhs: IVec2) {
		SubAssign::sub_assign(self.raw_mut(), rhs)
	}
}

// `Mul` operation forwarding

impl<M> Mul for TypedVectorImpl<IVec2, M>
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	type Output = Self;

	fn mul(self, rhs: Self) -> Self {
		self.map_raw(|lhs| Mul::mul(lhs, rhs.into_raw()))
	}
}

impl<M> Mul<IVec2> for TypedVectorImpl<IVec2, M>
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	type Output = Self;

	fn mul(self, rhs: IVec2) -> Self {
		self.map_raw(|lhs| Mul::mul(lhs, rhs))
	}
}

impl<M> Mul<i32> for TypedVectorImpl<IVec2, M>
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	type Output = Self;

	fn mul(self, rhs: i32) -> Self {
		self.map_raw(|lhs| Mul::mul(lhs, rhs))
	}
}

impl<M> Mul<TypedVectorImpl<IVec2, M>> for i32
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	type Output = TypedVectorImpl<IVec2, M>;

	fn mul(self, rhs: TypedVectorImpl<IVec2, M>) -> TypedVectorImpl<IVec2, M> {
		rhs.map_raw(|rhs| Mul::mul(self, rhs))
	}
}

impl<M> MulAssign for TypedVectorImpl<IVec2, M>
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	fn mul_assign(&mut self, rhs: Self) {
		MulAssign::mul_assign(self.raw_mut(), rhs.into_raw())
	}
}

impl<M> MulAssign<IVec2> for TypedVectorImpl<IVec2, M>
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	fn mul_assign(&mut self, rhs: IVec2) {
		MulAssign::mul_assign(self.raw_mut(), rhs)
	}
}

// `Div` operation forwarding

impl<M> Div for TypedVectorImpl<IVec2, M>
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	type Output = Self;

	fn div(self, rhs: Self) -> Self {
		self.map_raw(|lhs| Div::div(lhs, rhs.into_raw()))
	}
}

impl<M> Div<IVec2> for TypedVectorImpl<IVec2, M>
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	type Output = Self;

	fn div(self, rhs: IVec2) -> Self {
		self.map_raw(|lhs| Div::div(lhs, rhs))
	}
}

impl<M> Div<i32> for TypedVectorImpl<IVec2, M>
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	type Output = Self;

	fn div(self, rhs: i32) -> Self {
		self.map_raw(|lhs| Div::div(lhs, rhs))
	}
}

impl<M> Div<TypedVectorImpl<IVec2, M>> for i32
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	type Output = TypedVectorImpl<IVec2, M>;

	fn div(self, rhs: TypedVectorImpl<IVec2, M>) -> TypedVectorImpl<IVec2, M> {
		rhs.map_raw(|rhs| Div::div(self, rhs))
	}
}

impl<M> DivAssign for TypedVectorImpl<IVec2, M>
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	fn div_assign(&mut self, rhs: Self) {
		DivAssign::div_assign(self.raw_mut(), rhs.into_raw())
	}
}

impl<M> DivAssign<IVec2> for TypedVectorImpl<IVec2, M>
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	fn div_assign(&mut self, rhs: IVec2) {
		DivAssign::div_assign(self.raw_mut(), rhs)
	}
}

// `Rem` operation forwarding

impl<M> Rem for TypedVectorImpl<IVec2, M>
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	type Output = Self;

	fn rem(self, rhs: Self) -> Self {
		self.map_raw(|lhs| Rem::rem(lhs, rhs.into_raw()))
	}
}

impl<M> Rem<IVec2> for TypedVectorImpl<IVec2, M>
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	type Output = Self;

	fn rem(self, rhs: IVec2) -> Self {
		self.map_raw(|lhs| Rem::rem(lhs, rhs))
	}
}

impl<M> Rem<i32> for TypedVectorImpl<IVec2, M>
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	type Output = Self;

	fn rem(self, rhs: i32) -> Self {
		self.map_raw(|lhs| Rem::rem(lhs, rhs))
	}
}

impl<M> Rem<TypedVectorImpl<IVec2, M>> for i32
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	type Output = TypedVectorImpl<IVec2, M>;

	fn rem(self, rhs: TypedVectorImpl<IVec2, M>) -> TypedVectorImpl<IVec2, M> {
		rhs.map_raw(|rhs| Rem::rem(self, rhs))
	}
}

impl<M> RemAssign for TypedVectorImpl<IVec2, M>
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	fn rem_assign(&mut self, rhs: Self) {
		RemAssign::rem_assign(self.raw_mut(), rhs.into_raw())
	}
}

impl<M> RemAssign<IVec2> for TypedVectorImpl<IVec2, M>
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	fn rem_assign(&mut self, rhs: IVec2) {
		RemAssign::rem_assign(self.raw_mut(), rhs)
	}
}

// `BitAnd` operation forwarding

impl<M> BitAnd for TypedVectorImpl<IVec2, M>
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	type Output = Self;

	fn bitand(self, rhs: Self) -> Self {
		self.map_raw(|lhs| BitAnd::bitand(lhs, rhs.into_raw()))
	}
}

impl<M> BitAnd<IVec2> for TypedVectorImpl<IVec2, M>
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	type Output = Self;

	fn bitand(self, rhs: IVec2) -> Self {
		self.map_raw(|lhs| BitAnd::bitand(lhs, rhs))
	}
}

impl<M> BitAnd<i32> for TypedVectorImpl<IVec2, M>
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	type Output = Self;

	fn bitand(self, rhs: i32) -> Self {
		self.map_raw(|lhs| BitAnd::bitand(lhs, rhs))
	}
}

// `BitOr` operation forwarding

impl<M> BitOr for TypedVectorImpl<IVec2, M>
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	type Output = Self;

	fn bitor(self, rhs: Self) -> Self {
		self.map_raw(|lhs| BitOr::bitor(lhs, rhs.into_raw()))
	}
}

impl<M> BitOr<IVec2> for TypedVectorImpl<IVec2, M>
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	type Output = Self;

	fn bitor(self, rhs: IVec2) -> Self {
		self.map_raw(|lhs| BitOr::bitor(lhs, rhs))
	}
}

impl<M> BitOr<i32> for TypedVectorImpl<IVec2, M>
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	type Output = Self;

	fn bitor(self, rhs: i32) -> Self {
		self.map_raw(|lhs| BitOr::bitor(lhs, rhs))
	}
}

// `BitXor` operation forwarding

impl<M> BitXor for TypedVectorImpl<IVec2, M>
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	type Output = Self;

	fn bitxor(self, rhs: Self) -> Self {
		self.map_raw(|lhs| BitXor::bitxor(lhs, rhs.into_raw()))
	}
}

impl<M> BitXor<IVec2> for TypedVectorImpl<IVec2, M>
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	type Output = Self;

	fn bitxor(self, rhs: IVec2) -> Self {
		self.map_raw(|lhs| BitXor::bitxor(lhs, rhs))
	}
}

impl<M> BitXor<i32> for TypedVectorImpl<IVec2, M>
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	type Output = Self;

	fn bitxor(self, rhs: i32) -> Self {
		self.map_raw(|lhs| BitXor::bitxor(lhs, rhs))
	}
}

impl<M> Not for TypedVectorImpl<IVec2, M>
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	type Output = Self;

	fn not(self) -> Self {
		self.map_raw(|v| !v)
	}
}
impl<M> Neg for TypedVectorImpl<IVec2, M>
where
	M: ?Sized + VecFlavor<Backing = IVec2>,
{
	type Output = Self;

	fn neg(self) -> Self {
		self.map_raw(|v| -v)
	}
}
