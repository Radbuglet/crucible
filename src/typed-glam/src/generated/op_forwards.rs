use crate::backing_vec::Sealed;
use crate::{BackingVec, TypedVectorImpl, VecFlavor};
use core::convert::{AsMut, AsRef, From};
use core::ops::{
	Add, AddAssign, BitAnd, BitOr, BitXor, Div, DivAssign, Index, IndexMut, Mul, MulAssign, Neg,
	Not, Rem, RemAssign, Sub, SubAssign,
};
use glam::i32::IVec3;

// === Inherent `impl` items === //

impl<M> TypedVectorImpl<IVec3, M>
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	pub const ZERO: Self = Self::splat(0);
	pub const ONE: Self = Self::splat(1);

	pub const fn new(x: i32, y: i32, z: i32) -> Self {
		Self::from_raw(IVec3::new(x, y, z))
	}

	pub const fn splat(v: i32) -> Self {
		Self::from_raw(IVec3::splat(v))
	}
}

// === Misc trait derivations === //
// (most other traits are derived via trait logic in `lib.rs`)

impl BackingVec for IVec3 {}
impl Sealed for IVec3 {}

// Raw <-> Typed
impl<M> From<IVec3> for TypedVectorImpl<IVec3, M>
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	fn from(v: IVec3) -> Self {
		Self::from_raw(v)
	}
}

impl<M> From<TypedVectorImpl<IVec3, M>> for IVec3
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	fn from(v: TypedVectorImpl<IVec3, M>) -> Self {
		v.into_raw()
	}
}

// [i32; 3] <-> Typed
impl<M> From<[i32; 3]> for TypedVectorImpl<IVec3, M>
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	fn from(v: [i32; 3]) -> Self {
		IVec3::from(v).into()
	}
}

impl<M> From<TypedVectorImpl<IVec3, M>> for [i32; 3]
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	fn from(v: TypedVectorImpl<IVec3, M>) -> Self {
		v.into_raw().into()
	}
}

// (i32, ..., i32) <-> Typed
impl<M> From<(i32, i32, i32)> for TypedVectorImpl<IVec3, M>
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	fn from(v: (i32, i32, i32)) -> Self {
		IVec3::from(v).into()
	}
}

impl<M> From<TypedVectorImpl<IVec3, M>> for (i32, i32, i32)
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	fn from(v: TypedVectorImpl<IVec3, M>) -> Self {
		v.into_raw().into()
	}
}

// `AsRef` and `AsMut`
impl<M> AsRef<[i32; 3]> for TypedVectorImpl<IVec3, M>
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	fn as_ref(&self) -> &[i32; 3] {
		self.raw().as_ref()
	}
}

impl<M> AsMut<[i32; 3]> for TypedVectorImpl<IVec3, M>
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	fn as_mut(&mut self) -> &mut [i32; 3] {
		self.raw_mut().as_mut()
	}
}

// `Index` and `IndexMut`
impl<M> Index<usize> for TypedVectorImpl<IVec3, M>
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	type Output = i32;

	fn index(&self, i: usize) -> &i32 {
		&self.raw()[i]
	}
}

impl<M> IndexMut<usize> for TypedVectorImpl<IVec3, M>
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	fn index_mut(&mut self, i: usize) -> &mut i32 {
		&mut self.raw_mut()[i]
	}
}

// === `core::ops` trait forwards === //

// `Add` operation forwarding

impl<M> Add for TypedVectorImpl<IVec3, M>
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	type Output = Self;

	fn add(self, rhs: Self) -> Self {
		self.map_raw(|lhs| Add::add(lhs, rhs.into_raw()))
	}
}

impl<M> Add<IVec3> for TypedVectorImpl<IVec3, M>
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	type Output = Self;

	fn add(self, rhs: IVec3) -> Self {
		self.map_raw(|lhs| Add::add(lhs, rhs))
	}
}

impl<M> Add<i32> for TypedVectorImpl<IVec3, M>
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	type Output = Self;

	fn add(self, rhs: i32) -> Self {
		self.map_raw(|lhs| Add::add(lhs, rhs))
	}
}

impl<M> Add<TypedVectorImpl<IVec3, M>> for i32
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	type Output = TypedVectorImpl<IVec3, M>;

	fn add(self, rhs: TypedVectorImpl<IVec3, M>) -> TypedVectorImpl<IVec3, M> {
		rhs.map_raw(|rhs| Add::add(self, rhs))
	}
}

impl<M> AddAssign for TypedVectorImpl<IVec3, M>
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	fn add_assign(&mut self, rhs: Self) {
		AddAssign::add_assign(self.raw_mut(), rhs.into_raw())
	}
}

impl<M> AddAssign<IVec3> for TypedVectorImpl<IVec3, M>
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	fn add_assign(&mut self, rhs: IVec3) {
		AddAssign::add_assign(self.raw_mut(), rhs)
	}
}

// `Sub` operation forwarding

impl<M> Sub for TypedVectorImpl<IVec3, M>
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	type Output = Self;

	fn sub(self, rhs: Self) -> Self {
		self.map_raw(|lhs| Sub::sub(lhs, rhs.into_raw()))
	}
}

impl<M> Sub<IVec3> for TypedVectorImpl<IVec3, M>
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	type Output = Self;

	fn sub(self, rhs: IVec3) -> Self {
		self.map_raw(|lhs| Sub::sub(lhs, rhs))
	}
}

impl<M> Sub<i32> for TypedVectorImpl<IVec3, M>
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	type Output = Self;

	fn sub(self, rhs: i32) -> Self {
		self.map_raw(|lhs| Sub::sub(lhs, rhs))
	}
}

impl<M> Sub<TypedVectorImpl<IVec3, M>> for i32
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	type Output = TypedVectorImpl<IVec3, M>;

	fn sub(self, rhs: TypedVectorImpl<IVec3, M>) -> TypedVectorImpl<IVec3, M> {
		rhs.map_raw(|rhs| Sub::sub(self, rhs))
	}
}

impl<M> SubAssign for TypedVectorImpl<IVec3, M>
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	fn sub_assign(&mut self, rhs: Self) {
		SubAssign::sub_assign(self.raw_mut(), rhs.into_raw())
	}
}

impl<M> SubAssign<IVec3> for TypedVectorImpl<IVec3, M>
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	fn sub_assign(&mut self, rhs: IVec3) {
		SubAssign::sub_assign(self.raw_mut(), rhs)
	}
}

// `Mul` operation forwarding

impl<M> Mul for TypedVectorImpl<IVec3, M>
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	type Output = Self;

	fn mul(self, rhs: Self) -> Self {
		self.map_raw(|lhs| Mul::mul(lhs, rhs.into_raw()))
	}
}

impl<M> Mul<IVec3> for TypedVectorImpl<IVec3, M>
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	type Output = Self;

	fn mul(self, rhs: IVec3) -> Self {
		self.map_raw(|lhs| Mul::mul(lhs, rhs))
	}
}

impl<M> Mul<i32> for TypedVectorImpl<IVec3, M>
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	type Output = Self;

	fn mul(self, rhs: i32) -> Self {
		self.map_raw(|lhs| Mul::mul(lhs, rhs))
	}
}

impl<M> Mul<TypedVectorImpl<IVec3, M>> for i32
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	type Output = TypedVectorImpl<IVec3, M>;

	fn mul(self, rhs: TypedVectorImpl<IVec3, M>) -> TypedVectorImpl<IVec3, M> {
		rhs.map_raw(|rhs| Mul::mul(self, rhs))
	}
}

impl<M> MulAssign for TypedVectorImpl<IVec3, M>
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	fn mul_assign(&mut self, rhs: Self) {
		MulAssign::mul_assign(self.raw_mut(), rhs.into_raw())
	}
}

impl<M> MulAssign<IVec3> for TypedVectorImpl<IVec3, M>
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	fn mul_assign(&mut self, rhs: IVec3) {
		MulAssign::mul_assign(self.raw_mut(), rhs)
	}
}

// `Div` operation forwarding

impl<M> Div for TypedVectorImpl<IVec3, M>
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	type Output = Self;

	fn div(self, rhs: Self) -> Self {
		self.map_raw(|lhs| Div::div(lhs, rhs.into_raw()))
	}
}

impl<M> Div<IVec3> for TypedVectorImpl<IVec3, M>
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	type Output = Self;

	fn div(self, rhs: IVec3) -> Self {
		self.map_raw(|lhs| Div::div(lhs, rhs))
	}
}

impl<M> Div<i32> for TypedVectorImpl<IVec3, M>
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	type Output = Self;

	fn div(self, rhs: i32) -> Self {
		self.map_raw(|lhs| Div::div(lhs, rhs))
	}
}

impl<M> Div<TypedVectorImpl<IVec3, M>> for i32
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	type Output = TypedVectorImpl<IVec3, M>;

	fn div(self, rhs: TypedVectorImpl<IVec3, M>) -> TypedVectorImpl<IVec3, M> {
		rhs.map_raw(|rhs| Div::div(self, rhs))
	}
}

impl<M> DivAssign for TypedVectorImpl<IVec3, M>
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	fn div_assign(&mut self, rhs: Self) {
		DivAssign::div_assign(self.raw_mut(), rhs.into_raw())
	}
}

impl<M> DivAssign<IVec3> for TypedVectorImpl<IVec3, M>
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	fn div_assign(&mut self, rhs: IVec3) {
		DivAssign::div_assign(self.raw_mut(), rhs)
	}
}

// `Rem` operation forwarding

impl<M> Rem for TypedVectorImpl<IVec3, M>
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	type Output = Self;

	fn rem(self, rhs: Self) -> Self {
		self.map_raw(|lhs| Rem::rem(lhs, rhs.into_raw()))
	}
}

impl<M> Rem<IVec3> for TypedVectorImpl<IVec3, M>
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	type Output = Self;

	fn rem(self, rhs: IVec3) -> Self {
		self.map_raw(|lhs| Rem::rem(lhs, rhs))
	}
}

impl<M> Rem<i32> for TypedVectorImpl<IVec3, M>
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	type Output = Self;

	fn rem(self, rhs: i32) -> Self {
		self.map_raw(|lhs| Rem::rem(lhs, rhs))
	}
}

impl<M> Rem<TypedVectorImpl<IVec3, M>> for i32
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	type Output = TypedVectorImpl<IVec3, M>;

	fn rem(self, rhs: TypedVectorImpl<IVec3, M>) -> TypedVectorImpl<IVec3, M> {
		rhs.map_raw(|rhs| Rem::rem(self, rhs))
	}
}

impl<M> RemAssign for TypedVectorImpl<IVec3, M>
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	fn rem_assign(&mut self, rhs: Self) {
		RemAssign::rem_assign(self.raw_mut(), rhs.into_raw())
	}
}

impl<M> RemAssign<IVec3> for TypedVectorImpl<IVec3, M>
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	fn rem_assign(&mut self, rhs: IVec3) {
		RemAssign::rem_assign(self.raw_mut(), rhs)
	}
}

// `BitAnd` operation forwarding

impl<M> BitAnd for TypedVectorImpl<IVec3, M>
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	type Output = Self;

	fn bitand(self, rhs: Self) -> Self {
		self.map_raw(|lhs| BitAnd::bitand(lhs, rhs.into_raw()))
	}
}

impl<M> BitAnd<IVec3> for TypedVectorImpl<IVec3, M>
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	type Output = Self;

	fn bitand(self, rhs: IVec3) -> Self {
		self.map_raw(|lhs| BitAnd::bitand(lhs, rhs))
	}
}

impl<M> BitAnd<i32> for TypedVectorImpl<IVec3, M>
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	type Output = Self;

	fn bitand(self, rhs: i32) -> Self {
		self.map_raw(|lhs| BitAnd::bitand(lhs, rhs))
	}
}

// `BitOr` operation forwarding

impl<M> BitOr for TypedVectorImpl<IVec3, M>
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	type Output = Self;

	fn bitor(self, rhs: Self) -> Self {
		self.map_raw(|lhs| BitOr::bitor(lhs, rhs.into_raw()))
	}
}

impl<M> BitOr<IVec3> for TypedVectorImpl<IVec3, M>
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	type Output = Self;

	fn bitor(self, rhs: IVec3) -> Self {
		self.map_raw(|lhs| BitOr::bitor(lhs, rhs))
	}
}

impl<M> BitOr<i32> for TypedVectorImpl<IVec3, M>
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	type Output = Self;

	fn bitor(self, rhs: i32) -> Self {
		self.map_raw(|lhs| BitOr::bitor(lhs, rhs))
	}
}

// `BitXor` operation forwarding

impl<M> BitXor for TypedVectorImpl<IVec3, M>
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	type Output = Self;

	fn bitxor(self, rhs: Self) -> Self {
		self.map_raw(|lhs| BitXor::bitxor(lhs, rhs.into_raw()))
	}
}

impl<M> BitXor<IVec3> for TypedVectorImpl<IVec3, M>
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	type Output = Self;

	fn bitxor(self, rhs: IVec3) -> Self {
		self.map_raw(|lhs| BitXor::bitxor(lhs, rhs))
	}
}

impl<M> BitXor<i32> for TypedVectorImpl<IVec3, M>
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	type Output = Self;

	fn bitxor(self, rhs: i32) -> Self {
		self.map_raw(|lhs| BitXor::bitxor(lhs, rhs))
	}
}

impl<M> Not for TypedVectorImpl<IVec3, M>
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	type Output = Self;

	fn not(self) -> Self {
		self.map_raw(|v| !v)
	}
}
impl<M> Neg for TypedVectorImpl<IVec3, M>
where
	M: ?Sized + VecFlavor<Backing = IVec3>,
{
	type Output = Self;

	fn neg(self) -> Self {
		self.map_raw(|v| -v)
	}
}
