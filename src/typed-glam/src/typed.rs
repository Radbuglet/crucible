use bytemuck::TransparentWrapper;
use crucible_core::std_traits::ArrayLike;
use num_traits::Num;

use std::{
	any::type_name,
	fmt, hash,
	iter::{Product, Sum},
	ops::{self, Index, IndexMut},
};

use crate::traits::{
	floating_vector_forwards, numeric_vector_forwards, signed_vector_forwards, FloatingVector,
	FloatingVector2, FloatingVector3, FloatingVector4, GlamConvert, IntegerVector, NumericVector,
	NumericVector2, NumericVector3, NumericVector4, SignedNumericVector2, SignedNumericVector3,
	SignedNumericVector4, SignedVector,
};

// === Flavor traits === //

pub trait VecFlavor:
	FlavorCastFrom<TypedVector<Self>>
	+ FlavorCastFrom<Self::Backing>
	+ FlavorCastFrom<<Self::Backing as NumericVector>::Comp>
{
	type Backing: NumericVector;

	const NAME: &'static str;
}

pub trait FlavorCastFrom<V> {
	fn addend_from(vec: V) -> TypedVector<Self>
	where
		Self: VecFlavor;
}

pub macro vec_flavor($(
	$(#[$meta:meta])*
	$vis:vis struct $flavor:ident($backing:ty);
)*) {$(
	$(#[$meta])*
	$vis struct $flavor {
		_private: (),
	}

	impl VecFlavor for $flavor {
		type Backing = $backing;

		const NAME: &'static str = stringify!($flavor);
	}

	impl FlavorCastFrom<TypedVector<$flavor>> for $flavor {
		fn addend_from(vec: TypedVector<$flavor>) -> TypedVector<Self> {
			vec
		}
	}

	impl FlavorCastFrom<$backing> for $flavor {
		fn addend_from(vec: $backing) -> TypedVector<Self> {
			TypedVector::from_glam(vec)
		}
	}

	impl FlavorCastFrom<<$backing as NumericVector>::Comp> for $flavor {
		fn addend_from(comp: <$backing as NumericVector>::Comp) -> TypedVector<Self> {
			TypedVector::from_glam(NumericVector::splat(comp))
		}
	}
)*}

// === TypedVector === //

#[derive(TransparentWrapper)]
#[repr(transparent)]
pub struct TypedVector<F: ?Sized + VecFlavor>(F::Backing);

// GlamConvert
impl<F: ?Sized + VecFlavor> GlamConvert for TypedVector<F> {
	type Glam = F::Backing;

	fn to_glam(self) -> Self::Glam {
		self.0
	}

	fn from_glam(glam: Self::Glam) -> Self {
		Self(glam)
	}
}

// NumericVector
impl<F: ?Sized + VecFlavor> fmt::Debug for TypedVector<F> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_tuple(format!("TypedVector<{}>", type_name::<F>()).as_str())
			.field(&self.0)
			.finish()
	}
}

impl<F: ?Sized + VecFlavor> fmt::Display for TypedVector<F> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}({:?})", F::NAME, self.0.to_array().as_slice())
	}
}

impl<F: ?Sized + VecFlavor> Copy for TypedVector<F> {}

impl<F: ?Sized + VecFlavor> Clone for TypedVector<F> {
	fn clone(&self) -> Self {
		*self
	}
}

impl<F: ?Sized + VecFlavor> PartialEq for TypedVector<F> {
	fn eq(&self, other: &Self) -> bool {
		self.0 == other.0
	}
}

impl<F: ?Sized + VecFlavor> Default for TypedVector<F> {
	fn default() -> Self {
		Self(Default::default())
	}
}

impl<F: ?Sized + VecFlavor> Index<usize> for TypedVector<F> {
	type Output = <F::Backing as NumericVector>::Comp;

	fn index(&self, index: usize) -> &Self::Output {
		&self.0[index]
	}
}

impl<F: ?Sized + VecFlavor> IndexMut<usize> for TypedVector<F> {
	fn index_mut(&mut self, index: usize) -> &mut Self::Output {
		&mut self.0[index]
	}
}

impl<'a, F: ?Sized + VecFlavor> Sum<&'a Self> for TypedVector<F> {
	fn sum<I: Iterator<Item = &'a Self>>(iter: I) -> Self {
		Self::from_glam(F::Backing::sum(iter.map(|elem| &elem.0)))
	}
}

impl<'a, F: ?Sized + VecFlavor> Product<&'a Self> for TypedVector<F> {
	fn product<I: Iterator<Item = &'a Self>>(iter: I) -> Self {
		Self::from_glam(F::Backing::product(iter.map(|elem| &elem.0)))
	}
}

impl<B, F> NumericVector for TypedVector<F>
where
	B: ?Sized + NumericVector,
	F: ?Sized + VecFlavor<Backing = B>,
{
	numeric_vector_forwards!();

	type Comp = B::Comp;
	type CompArray = B::CompArray;
	type Mask = B::Mask;

	const DIM: usize = B::DIM;

	fn unit_axis(index: usize) -> Self {
		Self::unit_axis(index)
	}
}

impl<B, F> TypedVector<F>
where
	B: ?Sized + NumericVector,
	F: ?Sized + VecFlavor<Backing = B>,
{
	pub const DIM: usize = B::DIM;
	pub const ZERO: Self = Self(B::ZERO);
	pub const ONE: Self = Self(B::ONE);

	pub fn unit_axis(index: usize) -> Self {
		Self(B::unit_axis(index))
	}

	pub fn from_array(a: B::CompArray) -> Self {
		Self(B::from_array(a))
	}

	pub fn to_array(&self) -> B::CompArray {
		self.to_glam().to_array()
	}

	pub fn from_slice(slice: &[B::Comp]) -> Self {
		Self(B::from_slice(slice))
	}

	pub fn write_to_slice(self, slice: &mut [B::Comp]) {
		self.0.write_to_slice(slice)
	}

	pub fn splat(v: B::Comp) -> Self {
		Self(B::splat(v))
	}

	pub fn select(mask: B::Mask, if_true: Self, if_false: Self) -> Self {
		Self(B::select(mask, if_true.0, if_false.0))
	}

	pub fn min(self, rhs: Self) -> Self {
		self.map_glam(|lhs| lhs.min(rhs.0))
	}

	pub fn max(self, rhs: Self) -> Self {
		self.map_glam(|lhs| lhs.max(rhs.0))
	}

	pub fn clamp(self, min: Self, max: Self) -> Self {
		self.map_glam(|val| val.clamp(min.0, max.0))
	}

	pub fn min_element(self) -> B::Comp {
		self.0.min_element()
	}

	pub fn max_element(self) -> B::Comp {
		self.0.max_element()
	}

	pub fn cmpeq(self, rhs: Self) -> B::Mask {
		self.0.cmpeq(rhs.0)
	}

	pub fn cmpne(self, rhs: Self) -> B::Mask {
		self.0.cmpne(rhs.0)
	}

	pub fn cmpge(self, rhs: Self) -> B::Mask {
		self.0.cmpge(rhs.0)
	}

	pub fn cmpgt(self, rhs: Self) -> B::Mask {
		self.0.cmpgt(rhs.0)
	}

	pub fn cmple(self, rhs: Self) -> B::Mask {
		self.0.cmple(rhs.0)
	}

	pub fn cmplt(self, rhs: Self) -> B::Mask {
		self.0.cmplt(rhs.0)
	}

	pub fn dot(self, rhs: Self) -> B::Comp {
		self.0.dot(rhs.0)
	}
}

// IntegerVector
impl<F: ?Sized + VecFlavor> Eq for TypedVector<F> where F::Backing: IntegerVector {}

impl<F: ?Sized + VecFlavor> hash::Hash for TypedVector<F>
where
	F::Backing: IntegerVector,
{
	fn hash<H: hash::Hasher>(&self, state: &mut H) {
		self.0.hash(state);
	}
}

impl<B, F> IntegerVector for TypedVector<F>
where
	B: ?Sized + IntegerVector,
	F: ?Sized + VecFlavor<Backing = B>,
{
	fn shl_prim<N: Num>(self, v: N) -> Self {
		self.shl_prim(v)
	}

	fn shr_prim<N: Num>(self, v: N) -> Self {
		self.shr_prim(v)
	}
}

impl<B, F> TypedVector<F>
where
	B: ?Sized + IntegerVector,
	F: ?Sized + VecFlavor<Backing = B>,
{
	fn shl_prim<N: Num>(self, v: N) -> Self {
		self.map_glam(|raw| raw.shl_prim(v))
	}

	fn shr_prim<N: Num>(self, v: N) -> Self {
		self.map_glam(|raw| raw.shr_prim(v))
	}
}

// SignedVector

impl<B, F> SignedVector for TypedVector<F>
where
	B: ?Sized + SignedVector,
	F: ?Sized + VecFlavor<Backing = B>,
{
	signed_vector_forwards!();
}

impl<B, F> TypedVector<F>
where
	B: ?Sized + SignedVector,
	F: ?Sized + VecFlavor<Backing = B>,
{
	pub const NEG_ONE: Self = Self(B::NEG_ONE);

	pub fn abs(self) -> Self {
		self.map_glam(|raw| raw.abs())
	}

	pub fn signum(self) -> Self {
		self.map_glam(|raw| raw.signum())
	}
}

// FloatingVector
impl<B, F> FloatingVector for TypedVector<F>
where
	B: ?Sized + FloatingVector,
	F: ?Sized + VecFlavor<Backing = B>,
{
	floating_vector_forwards!();
}

impl<B, F> TypedVector<F>
where
	B: ?Sized + FloatingVector,
	F: ?Sized + VecFlavor<Backing = B>,
{
	pub const NAN: Self = Self(B::NAN);

	pub fn is_finite(self) -> bool {
		self.0.is_finite()
	}

	pub fn is_nan(self) -> bool {
		self.0.is_nan()
	}

	pub fn is_nan_mask(self) -> B::Mask {
		self.0.is_nan_mask()
	}

	pub fn length(self) -> B::Comp {
		self.0.length()
	}

	pub fn length_squared(self) -> B::Comp {
		self.0.length_squared()
	}

	pub fn length_recip(self) -> B::Comp {
		self.0.length_recip()
	}

	pub fn distance(self, rhs: Self) -> B::Comp {
		self.0.distance(rhs.0)
	}

	pub fn distance_squared(self, rhs: Self) -> B::Comp {
		self.0.distance_squared(rhs.0)
	}

	pub fn normalize(self) -> Self {
		self.map_glam(|raw| raw.normalize())
	}

	pub fn try_normalize(self) -> Option<Self> {
		Some(Self(self.0.try_normalize()?))
	}

	pub fn normalize_or_zero(self) -> Self {
		self.map_glam(|raw| raw.normalize_or_zero())
	}

	pub fn is_normalized(self) -> bool {
		self.0.is_normalized()
	}

	pub fn project_onto(self, rhs: Self) -> Self {
		self.map_glam(|raw| raw.project_onto(rhs.0))
	}

	pub fn reject_from(self, rhs: Self) -> Self {
		self.map_glam(|raw| raw.reject_from(rhs.0))
	}

	pub fn project_onto_normalized(self, rhs: Self) -> Self {
		self.map_glam(|raw| raw.project_onto_normalized(rhs.0))
	}

	pub fn reject_from_normalized(self, rhs: Self) -> Self {
		self.map_glam(|raw| raw.reject_from_normalized(rhs.0))
	}

	pub fn round(self) -> Self {
		self.map_glam(|raw| raw.round())
	}

	pub fn floor(self) -> Self {
		self.map_glam(|raw| raw.floor())
	}

	pub fn ceil(self) -> Self {
		self.map_glam(|raw| raw.ceil())
	}

	pub fn fract(self) -> Self {
		self.map_glam(|raw| raw.fract())
	}

	pub fn exp(self) -> Self {
		self.map_glam(|raw| raw.exp())
	}

	pub fn powf(self, n: B::Comp) -> Self {
		self.map_glam(|raw| raw.powf(n))
	}

	pub fn recip(self) -> Self {
		self.map_glam(|raw| raw.recip())
	}

	pub fn lerp(self, rhs: Self, s: B::Comp) -> Self {
		self.map_glam(|raw| raw.lerp(rhs.0, s))
	}

	pub fn abs_diff_eq(self, rhs: Self, max_abs_diff: B::Comp) -> bool {
		self.0.abs_diff_eq(rhs.0, max_abs_diff)
	}

	pub fn clamp_length(self, min: B::Comp, max: B::Comp) -> Self {
		self.map_glam(|raw| raw.clamp_length(min, max))
	}

	pub fn clamp_length_max(self, max: B::Comp) -> Self {
		self.map_glam(|raw| raw.clamp_length_max(max))
	}

	pub fn clamp_length_min(self, min: B::Comp) -> Self {
		self.map_glam(|raw| raw.clamp_length_min(min))
	}

	pub fn mul_add(self, a: Self, b: Self) -> Self {
		self.map_glam(|raw| raw.mul_add(a.0, b.0))
	}
}

// NumericVector2
// TODO: Make this, and other variadic traits, inherent
impl<B, F> From<(B::Comp, B::Comp)> for TypedVector<F>
where
	B: ?Sized + NumericVector2,
	F: ?Sized + VecFlavor<Backing = B>,
{
	fn from(tup: (B::Comp, B::Comp)) -> Self {
		Self(B::from(tup))
	}
}

impl<B, F> From<TypedVector<F>> for (B::Comp, B::Comp)
where
	B: ?Sized + NumericVector2,
	F: ?Sized + VecFlavor<Backing = B>,
{
	fn from(vec: TypedVector<F>) -> Self {
		vec.0.into()
	}
}

impl<B, F> NumericVector2 for TypedVector<F>
where
	B: ?Sized + NumericVector2,
	F: ?Sized + VecFlavor<Backing = B>,
{
	const X: Self = Self(B::X);
	const Y: Self = Self(B::Y);

	fn new(x: B::Comp, y: B::Comp) -> Self {
		Self(B::new(x, y))
	}
}

// SignedNumericVector2
impl<B, F> SignedNumericVector2 for TypedVector<F>
where
	B: ?Sized + SignedNumericVector2,
	F: ?Sized + VecFlavor<Backing = B>,
{
	const NEG_X: Self = Self(B::NEG_X);
	const NEG_Y: Self = Self(B::NEG_Y);
}

// NumericVector3
impl<B, F> From<(B::Comp, B::Comp, B::Comp)> for TypedVector<F>
where
	B: ?Sized + NumericVector3,
	F: ?Sized + VecFlavor<Backing = B>,
{
	fn from(tup: (B::Comp, B::Comp, B::Comp)) -> Self {
		Self(B::from(tup))
	}
}

impl<B, F> From<TypedVector<F>> for (B::Comp, B::Comp, B::Comp)
where
	B: ?Sized + NumericVector3,
	F: ?Sized + VecFlavor<Backing = B>,
{
	fn from(vec: TypedVector<F>) -> Self {
		vec.0.into()
	}
}

impl<B, F> NumericVector3 for TypedVector<F>
where
	B: ?Sized + NumericVector3,
	F: ?Sized + VecFlavor<Backing = B>,
{
	const X: Self = Self(B::X);
	const Y: Self = Self(B::Y);
	const Z: Self = Self(B::Z);

	fn new(x: B::Comp, y: B::Comp, z: B::Comp) -> Self {
		Self(B::new(x, y, z))
	}

	fn cross(self, rhs: Self) -> Self {
		self.map_glam(|raw| raw.cross(rhs.0))
	}
}

// SignedNumericVector3
impl<B, F> SignedNumericVector3 for TypedVector<F>
where
	B: ?Sized + SignedNumericVector3,
	F: ?Sized + VecFlavor<Backing = B>,
{
	const NEG_X: Self = Self(B::NEG_X);
	const NEG_Y: Self = Self(B::NEG_Y);
	const NEG_Z: Self = Self(B::NEG_Z);
}

// NumericVector4
impl<B, F> From<(B::Comp, B::Comp, B::Comp, B::Comp)> for TypedVector<F>
where
	B: ?Sized + NumericVector4,
	F: ?Sized + VecFlavor<Backing = B>,
{
	fn from(tup: (B::Comp, B::Comp, B::Comp, B::Comp)) -> Self {
		Self(B::from(tup))
	}
}

impl<B, F> From<TypedVector<F>> for (B::Comp, B::Comp, B::Comp, B::Comp)
where
	B: ?Sized + NumericVector4,
	F: ?Sized + VecFlavor<Backing = B>,
{
	fn from(vec: TypedVector<F>) -> Self {
		vec.0.into()
	}
}

impl<B, F> NumericVector4 for TypedVector<F>
where
	B: ?Sized + NumericVector4,
	F: ?Sized + VecFlavor<Backing = B>,
{
	const X: Self = Self(B::X);
	const Y: Self = Self(B::Y);
	const Z: Self = Self(B::Z);
	const W: Self = Self(B::W);

	fn new(x: Self::Comp, y: Self::Comp, z: Self::Comp, w: Self::Comp) -> Self {
		Self(B::new(x, y, z, w))
	}
}

// SignedNumericVector4
impl<B, F> SignedNumericVector4 for TypedVector<F>
where
	B: ?Sized + SignedNumericVector4,
	F: ?Sized + VecFlavor<Backing = B>,
{
	const NEG_X: Self = Self(B::NEG_X);
	const NEG_Y: Self = Self(B::NEG_Y);
	const NEG_Z: Self = Self(B::NEG_Z);
	const NEG_W: Self = Self(B::NEG_W);
}

// FloatingVector2
impl<B, F> FloatingVector2 for TypedVector<F>
where
	B: ?Sized + FloatingVector2,
	F: ?Sized + VecFlavor<Backing = B>,
{
	fn from_angle(angle: Self::Comp) -> Self {
		Self(B::from_angle(angle))
	}

	fn angle_between(self, rhs: Self) -> Self::Comp {
		self.0.angle_between(rhs.0)
	}

	fn perp(self) -> Self {
		self.map_glam(|raw| raw.perp())
	}

	fn perp_dot(self, rhs: Self) -> Self::Comp {
		self.0.perp_dot(rhs.0)
	}

	fn rotate(self, rhs: Self) -> Self {
		self.map_glam(|raw| raw.rotate(rhs.0))
	}
}

// FloatingVector3
impl<B, F> FloatingVector3 for TypedVector<F>
where
	B: ?Sized + FloatingVector3,
	F: ?Sized + VecFlavor<Backing = B>,
{
	fn angle_between(self, rhs: Self) -> Self::Comp {
		self.0.angle_between(rhs.0)
	}

	fn any_orthogonal_vector(&self) -> Self {
		Self(self.0.any_orthogonal_vector())
	}

	fn any_orthonormal_vector(&self) -> Self {
		Self(self.0.any_orthonormal_vector())
	}

	fn any_orthonormal_pair(&self) -> (Self, Self) {
		let (a, b) = self.0.any_orthonormal_pair();
		(Self(a), Self(b))
	}
}

// FloatingVector4
impl<B, F> FloatingVector4 for TypedVector<F>
where
	B: ?Sized + FloatingVector4,
	F: ?Sized + VecFlavor<Backing = B>,
{
}

// Overload derivations
macro derive_bin_ops(
	$(
		$trait:ident, $fn:ident
		$(, $trait_assign:ident, $fn_assign:ident)?
		$(for $extra_trait:ident)?
	);*
	$(;)?
) {$(
	impl<B, F, R> ops::$trait<R> for TypedVector<F>
	where
		B: ?Sized + NumericVector $(+ $extra_trait)?,
		F: ?Sized + VecFlavor<Backing = B> + FlavorCastFrom<R>,
	{
		type Output = Self;

		fn $fn(self, rhs: R) -> Self::Output {
			self.map_glam(|lhs| ops::$trait::$fn(lhs, F::addend_from(rhs).to_glam()))
		}
	}

	$(
		impl<F, R> ops::$trait_assign<R> for TypedVector<F>
		where
			// N.B. Yes, these trait bounds are wrong. Luckily, additional bounds
			// are never used with the assign variant so we're *fine* for now.
			F: ?Sized + VecFlavor + FlavorCastFrom<R>,
		{
			fn $fn_assign(&mut self, rhs: R) {
				ops::$trait_assign::$fn_assign(&mut self.0, F::addend_from(rhs).to_glam());
			}
		}
	)?
)*}

derive_bin_ops!(
	// NumericVector
	Add, add, AddAssign, add_assign;
	Sub, sub, SubAssign, sub_assign;
	Mul, mul, MulAssign, mul_assign;
	Div, div, DivAssign, div_assign;
	Rem, rem, RemAssign, rem_assign;

	// IntegerVector
	BitAnd, bitand for IntegerVector;
	BitOr, bitor for IntegerVector;
	BitXor, bitxor for IntegerVector;
	Shl, shl for IntegerVector;
	Shr, shr for IntegerVector;
);

// IntegerVector
impl<B, F> ops::Not for TypedVector<F>
where
	B: ?Sized + IntegerVector,
	F: ?Sized + VecFlavor<Backing = B>,
{
	type Output = Self;

	fn not(self) -> Self::Output {
		Self(ops::Not::not(self.0))
	}
}

// SignedVector
impl<B, F> ops::Neg for TypedVector<F>
where
	B: ?Sized + SignedVector,
	F: ?Sized + VecFlavor<Backing = B>,
{
	type Output = Self;

	fn neg(self) -> Self::Output {
		Self(ops::Neg::neg(self.0))
	}
}
