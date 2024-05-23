use std::{
    fmt, hash,
    iter::{Product, Sum},
    marker::PhantomData,
    ops::{self, Index, IndexMut},
};

use newtypes::transparent;
use std_traits::ArrayLike;

use crate::traits::{
    floating_vector_forwards, numeric_vector_forwards, signed_vector_forwards, CastVecFrom, Dim2,
    Dim3, Dim4, DimClass, FloatingVector, FloatingVector2, FloatingVector3, FloatingVector4,
    GlamBacked, IntegerVector, NumericVector, NumericVector2, NumericVector3, NumericVector4,
    SignedNumericVector2, SignedNumericVector3, SignedNumericVector4, SignedVector,
};

// === Flavor traits === //

pub trait VecFlavor: 'static + FlavorCastFrom<TypedVector<Self>> {
    type Backing: NumericVector;

    const DEBUG_NAME: &'static str;
}

pub trait FlavorCastFrom<V> {
    fn cast_from(vec: V) -> TypedVector<Self>
    where
        Self: VecFlavor;
}

impl<F: ?Sized + VecFlavor> FlavorCastFrom<TypedVector<F>> for F {
    fn cast_from(vec: TypedVector<F>) -> TypedVector<Self> {
        vec
    }
}

// === TypedVector === //

pub type TypedVector<F> = TypedVectorImpl<F, <<F as VecFlavor>::Backing as NumericVector>::Dim>;

#[transparent(raw)]
#[repr(transparent)]
pub struct TypedVectorImpl<F, D>
where
    F: ?Sized + VecFlavor,
    D: DimClass,
{
    _ty: PhantomData<fn(D) -> D>,
    raw: F::Backing,
}

// `VecFrom` and `NumericVector`
impl<F: ?Sized + VecFlavor> TypedVector<F> {
    pub fn cast_from<T>(v: T) -> Self
    where
        Self: CastVecFrom<T>,
    {
        <Self as CastVecFrom<T>>::cast_from(v)
    }

    pub fn cast<T: CastVecFrom<Self>>(self) -> T {
        T::cast_from(self)
    }
}

impl<F, V> CastVecFrom<V> for TypedVector<F>
where
    F: ?Sized + VecFlavor,
    F: FlavorCastFrom<V>,
{
    fn cast_from(other: V) -> Self {
        F::cast_from(other)
    }
}

// GlamConvert
impl<F: ?Sized + VecFlavor> GlamBacked for TypedVector<F> {
    type Glam = F::Backing;

    fn to_glam(self) -> Self::Glam {
        self.to_glam()
    }

    fn as_glam(&self) -> &Self::Glam {
        self.as_glam()
    }

    fn as_glam_mut(&mut self) -> &mut Self::Glam {
        self.as_glam_mut()
    }

    fn from_glam(glam: Self::Glam) -> Self {
        Self::from_glam(glam)
    }

    fn from_glam_ref(glam: &Self::Glam) -> &Self {
        Self::from_glam_ref(glam)
    }

    fn from_glam_mut(glam: &mut Self::Glam) -> &mut Self {
        Self::from_glam_mut(glam)
    }
}

impl<F: ?Sized + VecFlavor> TypedVector<F> {
    pub fn to_glam(self) -> F::Backing {
        self.raw
    }

    pub fn as_glam(&self) -> &F::Backing {
        &self.raw
    }

    pub fn as_glam_mut(&mut self) -> &mut F::Backing {
        &mut self.raw
    }

    pub const fn from_glam(raw: F::Backing) -> Self {
        Self {
            _ty: PhantomData,
            raw,
        }
    }

    pub fn from_glam_ref(glam: &F::Backing) -> &Self {
        Self::transparent_from_ref(glam)
    }

    pub fn from_glam_mut(glam: &mut F::Backing) -> &mut Self {
        Self::transparent_from_mut(glam)
    }

    // Copied from `GlamConvert`
    pub fn map_glam<R, C>(self, f: C) -> R
    where
        R: GlamBacked,
        C: FnOnce(F::Backing) -> R::Glam,
    {
        R::from_glam(f(self.to_glam()))
    }

    pub fn cast_glam<T: GlamBacked<Glam = F::Backing>>(self) -> T {
        T::from_glam(self.to_glam())
    }

    pub fn cast_glam_ref<T: GlamBacked<Glam = F::Backing>>(&self) -> &T {
        T::from_glam_ref(self.as_glam())
    }

    pub fn cast_glam_mut<T: GlamBacked<Glam = F::Backing>>(&mut self) -> &mut T {
        T::from_glam_mut(self.as_glam_mut())
    }
}

// NumericVector
impl<F: ?Sized + VecFlavor> fmt::Debug for TypedVector<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self, f)
    }
}

impl<F: ?Sized + VecFlavor> fmt::Display for TypedVector<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}({:?})",
            F::DEBUG_NAME,
            self.as_glam().to_array().as_slice()
        )
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
        self.as_glam() == other.as_glam()
    }
}

impl<F: ?Sized + VecFlavor> Default for TypedVector<F> {
    fn default() -> Self {
        Self::from_glam(Default::default())
    }
}

impl<F: ?Sized + VecFlavor> Index<usize> for TypedVector<F> {
    type Output = <F::Backing as NumericVector>::Comp;

    fn index(&self, index: usize) -> &Self::Output {
        &self.as_glam()[index]
    }
}

impl<F: ?Sized + VecFlavor> IndexMut<usize> for TypedVector<F> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.as_glam_mut()[index]
    }
}

impl<'a, F: ?Sized + VecFlavor> Sum<&'a Self> for TypedVector<F> {
    fn sum<I: Iterator<Item = &'a Self>>(iter: I) -> Self {
        Self::from_glam(F::Backing::sum(iter.map(|elem| elem.as_glam())))
    }
}

impl<'a, F: ?Sized + VecFlavor> Product<&'a Self> for TypedVector<F> {
    fn product<I: Iterator<Item = &'a Self>>(iter: I) -> Self {
        Self::from_glam(F::Backing::product(iter.map(|elem| elem.as_glam())))
    }
}

impl<B, F> NumericVector for TypedVector<F>
where
    B: NumericVector,
    F: ?Sized + VecFlavor<Backing = B>,
{
    numeric_vector_forwards!();

    type Dim = B::Dim;
    type Comp = B::Comp;
    type CompArray = B::CompArray;
    type Mask = B::Mask;

    fn unit_axis(index: usize) -> Self {
        Self::unit_axis(index)
    }
}

impl<B, F> TypedVector<F>
where
    B: NumericVector,
    F: ?Sized + VecFlavor<Backing = B>,
{
    pub const ZERO: Self = Self::from_glam(B::ZERO);

    pub const ONE: Self = Self::from_glam(B::ONE);

    pub fn unit_axis(index: usize) -> Self {
        Self::from_glam(B::unit_axis(index))
    }

    pub fn from_array(a: B::CompArray) -> Self {
        Self::from_glam(B::from_array(a))
    }

    pub fn to_array(&self) -> B::CompArray {
        self.to_glam().to_array()
    }

    pub fn from_slice(slice: &[B::Comp]) -> Self {
        Self::from_glam(B::from_slice(slice))
    }

    pub fn write_to_slice(self, slice: &mut [B::Comp]) {
        self.to_glam().write_to_slice(slice)
    }

    pub fn splat(v: B::Comp) -> Self {
        Self::from_glam(B::splat(v))
    }

    pub fn select(mask: B::Mask, if_true: Self, if_false: Self) -> Self {
        Self::from_glam(B::select(mask, if_true.to_glam(), if_false.to_glam()))
    }

    pub fn min(self, rhs: Self) -> Self {
        self.map_glam(|lhs| lhs.min(rhs.to_glam()))
    }

    pub fn max(self, rhs: Self) -> Self {
        self.map_glam(|lhs| lhs.max(rhs.to_glam()))
    }

    pub fn clamp(self, min: Self, max: Self) -> Self {
        self.map_glam(|val| val.clamp(min.to_glam(), max.to_glam()))
    }

    pub fn min_element(self) -> B::Comp {
        self.to_glam().min_element()
    }

    pub fn max_element(self) -> B::Comp {
        self.to_glam().max_element()
    }

    pub fn cmpeq(self, rhs: Self) -> B::Mask {
        self.to_glam().cmpeq(rhs.to_glam())
    }

    pub fn cmpne(self, rhs: Self) -> B::Mask {
        self.to_glam().cmpne(rhs.to_glam())
    }

    pub fn cmpge(self, rhs: Self) -> B::Mask {
        self.to_glam().cmpge(rhs.to_glam())
    }

    pub fn cmpgt(self, rhs: Self) -> B::Mask {
        self.to_glam().cmpgt(rhs.to_glam())
    }

    pub fn cmple(self, rhs: Self) -> B::Mask {
        self.to_glam().cmple(rhs.to_glam())
    }

    pub fn cmplt(self, rhs: Self) -> B::Mask {
        self.to_glam().cmplt(rhs.to_glam())
    }

    pub fn dot(self, rhs: Self) -> B::Comp {
        self.to_glam().dot(rhs.to_glam())
    }

    pub fn dot_into_vec(self, rhs: Self) -> Self {
        self.map_glam(|v| v.dot_into_vec(rhs.to_glam()))
    }
}

// IntegerVector
impl<F: ?Sized + VecFlavor> Eq for TypedVector<F> where F::Backing: IntegerVector {}

impl<F: ?Sized + VecFlavor> hash::Hash for TypedVector<F>
where
    F::Backing: IntegerVector,
{
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.as_glam().hash(state);
    }
}

impl<B, F> IntegerVector for TypedVector<F>
where
    B: ?Sized + IntegerVector,
    F: ?Sized + VecFlavor<Backing = B>,
{
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
    pub const NEG_ONE: Self = Self::from_glam(B::NEG_ONE);

    pub fn abs(self) -> Self {
        self.map_glam(|raw| raw.abs())
    }

    pub fn signum(self) -> Self {
        self.map_glam(|raw| raw.signum())
    }

    pub fn is_negative_bitmask(self) -> u32 {
        self.to_glam().is_negative_bitmask()
    }

    fn copysign(self, rhs: Self) -> Self {
        self.map_glam(|v| v.copysign(rhs.to_glam()))
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
    pub const NAN: Self = Self::from_glam(B::NAN);

    pub fn is_finite(self) -> bool {
        self.to_glam().is_finite()
    }

    pub fn is_nan(self) -> bool {
        self.to_glam().is_nan()
    }

    pub fn is_nan_mask(self) -> B::Mask {
        self.to_glam().is_nan_mask()
    }

    pub fn length(self) -> B::Comp {
        self.to_glam().length()
    }

    pub fn length_squared(self) -> B::Comp {
        self.to_glam().length_squared()
    }

    pub fn length_recip(self) -> B::Comp {
        self.to_glam().length_recip()
    }

    pub fn distance(self, rhs: Self) -> B::Comp {
        self.to_glam().distance(rhs.to_glam())
    }

    pub fn distance_squared(self, rhs: Self) -> B::Comp {
        self.to_glam().distance_squared(rhs.to_glam())
    }

    pub fn normalize(self) -> Self {
        self.map_glam(|raw| raw.normalize())
    }

    pub fn try_normalize(self) -> Option<Self> {
        Some(Self::from_glam(self.to_glam().try_normalize()?))
    }

    pub fn normalize_or_zero(self) -> Self {
        self.map_glam(|raw| raw.normalize_or_zero())
    }

    pub fn is_normalized(self) -> bool {
        self.to_glam().is_normalized()
    }

    pub fn project_onto(self, rhs: Self) -> Self {
        self.map_glam(|raw| raw.project_onto(rhs.to_glam()))
    }

    pub fn reject_from(self, rhs: Self) -> Self {
        self.map_glam(|raw| raw.reject_from(rhs.to_glam()))
    }

    pub fn project_onto_normalized(self, rhs: Self) -> Self {
        self.map_glam(|raw| raw.project_onto_normalized(rhs.to_glam()))
    }

    pub fn reject_from_normalized(self, rhs: Self) -> Self {
        self.map_glam(|raw| raw.reject_from_normalized(rhs.to_glam()))
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
        self.map_glam(|raw| raw.lerp(rhs.to_glam(), s))
    }

    pub fn abs_diff_eq(self, rhs: Self, max_abs_diff: B::Comp) -> bool {
        self.to_glam().abs_diff_eq(rhs.to_glam(), max_abs_diff)
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
        self.map_glam(|raw| raw.mul_add(a.to_glam(), b.to_glam()))
    }
}

// NumericVector2
impl<B, F> From<(B::Comp, B::Comp)> for TypedVector<F>
where
    B: NumericVector2,
    F: ?Sized + VecFlavor<Backing = B>,
{
    fn from(tup: (B::Comp, B::Comp)) -> Self {
        Self::from_glam(B::from(tup))
    }
}

impl<B, F> From<TypedVector<F>> for (B::Comp, B::Comp)
where
    B: NumericVector2,
    F: ?Sized + VecFlavor<Backing = B>,
{
    fn from(vec: TypedVector<F>) -> Self {
        vec.to_glam().into()
    }
}

impl<B, F> TypedVectorImpl<F, Dim2>
where
    B: NumericVector2,
    F: ?Sized + VecFlavor<Backing = B>,
{
    pub const X: Self = Self::from_glam(B::X);
    pub const Y: Self = Self::from_glam(B::Y);

    pub fn new(x: B::Comp, y: B::Comp) -> Self {
        Self::from_glam(B::new(x, y))
    }

    pub fn x(&self) -> B::Comp {
        self.as_glam().x()
    }

    pub fn x_mut(&mut self) -> &mut B::Comp {
        self.as_glam_mut().x_mut()
    }

    pub fn y(&self) -> B::Comp {
        self.as_glam().y()
    }

    pub fn y_mut(&mut self) -> &mut B::Comp {
        self.as_glam_mut().y_mut()
    }
}

impl<B, F> NumericVector2 for TypedVector<F>
where
    B: NumericVector2,
    F: ?Sized + VecFlavor<Backing = B>,
{
    const X: Self = Self::X;
    const Y: Self = Self::Y;

    fn new(x: B::Comp, y: B::Comp) -> Self {
        Self::new(x, y)
    }

    fn x(&self) -> Self::Comp {
        self.x()
    }

    fn x_mut(&mut self) -> &mut Self::Comp {
        self.x_mut()
    }

    fn y(&self) -> Self::Comp {
        self.y()
    }

    fn y_mut(&mut self) -> &mut Self::Comp {
        self.y_mut()
    }
}

// SignedNumericVector2
impl<B, F> TypedVectorImpl<F, Dim2>
where
    B: ?Sized + SignedNumericVector2,
    F: ?Sized + VecFlavor<Backing = B>,
{
    pub const NEG_X: Self = Self::from_glam(B::NEG_X);
    pub const NEG_Y: Self = Self::from_glam(B::NEG_Y);
}

impl<B, F> SignedNumericVector2 for TypedVector<F>
where
    B: ?Sized + SignedNumericVector2,
    F: ?Sized + VecFlavor<Backing = B>,
{
    const NEG_X: Self = Self::NEG_X;
    const NEG_Y: Self = Self::NEG_Y;
}

// NumericVector3
impl<B, F> From<(B::Comp, B::Comp, B::Comp)> for TypedVector<F>
where
    B: NumericVector3,
    F: ?Sized + VecFlavor<Backing = B>,
{
    fn from(tup: (B::Comp, B::Comp, B::Comp)) -> Self {
        Self::from_glam(B::from(tup))
    }
}

impl<B, F> From<TypedVector<F>> for (B::Comp, B::Comp, B::Comp)
where
    B: NumericVector3,
    F: ?Sized + VecFlavor<Backing = B>,
{
    fn from(vec: TypedVector<F>) -> Self {
        vec.to_glam().into()
    }
}

impl<B, F> TypedVectorImpl<F, Dim3>
where
    B: NumericVector3,
    F: ?Sized + VecFlavor<Backing = B>,
{
    pub const X: Self = Self::from_glam(B::X);
    pub const Y: Self = Self::from_glam(B::Y);
    pub const Z: Self = Self::from_glam(B::Z);

    pub fn new(x: B::Comp, y: B::Comp, z: B::Comp) -> Self {
        Self::from_glam(B::new(x, y, z))
    }

    pub fn cross(self, rhs: Self) -> Self {
        self.map_glam(|raw| raw.cross(rhs.to_glam()))
    }

    pub fn x(&self) -> B::Comp {
        self.as_glam().x()
    }

    pub fn x_mut(&mut self) -> &mut B::Comp {
        self.as_glam_mut().x_mut()
    }

    pub fn y(&self) -> B::Comp {
        self.as_glam().y()
    }

    pub fn y_mut(&mut self) -> &mut B::Comp {
        self.as_glam_mut().y_mut()
    }

    pub fn z(&self) -> B::Comp {
        self.as_glam().z()
    }

    pub fn z_mut(&mut self) -> &mut B::Comp {
        self.as_glam_mut().z_mut()
    }
}

impl<B, F> NumericVector3 for TypedVector<F>
where
    B: NumericVector3,
    F: ?Sized + VecFlavor<Backing = B>,
{
    const X: Self = Self::X;
    const Y: Self = Self::Y;
    const Z: Self = Self::Z;

    fn new(x: B::Comp, y: B::Comp, z: B::Comp) -> Self {
        Self::new(x, y, z)
    }

    fn cross(self, rhs: Self) -> Self {
        self.cross(rhs)
    }

    fn x(&self) -> Self::Comp {
        self.x()
    }

    fn x_mut(&mut self) -> &mut Self::Comp {
        self.x_mut()
    }

    fn y(&self) -> Self::Comp {
        self.y()
    }

    fn y_mut(&mut self) -> &mut Self::Comp {
        self.y_mut()
    }

    fn z(&self) -> Self::Comp {
        self.z()
    }

    fn z_mut(&mut self) -> &mut Self::Comp {
        self.z_mut()
    }
}

// SignedNumericVector3
impl<B, F> TypedVectorImpl<F, Dim3>
where
    B: ?Sized + SignedNumericVector3,
    F: ?Sized + VecFlavor<Backing = B>,
{
    pub const NEG_X: Self = Self::from_glam(B::NEG_X);
    pub const NEG_Y: Self = Self::from_glam(B::NEG_Y);
    pub const NEG_Z: Self = Self::from_glam(B::NEG_Z);
}

impl<B, F> SignedNumericVector3 for TypedVector<F>
where
    B: ?Sized + SignedNumericVector3,
    F: ?Sized + VecFlavor<Backing = B>,
{
    const NEG_X: Self = Self::NEG_X;
    const NEG_Y: Self = Self::NEG_Y;
    const NEG_Z: Self = Self::NEG_Z;
}

// NumericVector4
impl<B, F> From<(B::Comp, B::Comp, B::Comp, B::Comp)> for TypedVector<F>
where
    B: NumericVector4,
    F: ?Sized + VecFlavor<Backing = B>,
{
    fn from(tup: (B::Comp, B::Comp, B::Comp, B::Comp)) -> Self {
        Self::from_glam(B::from(tup))
    }
}

impl<B, F> From<TypedVector<F>> for (B::Comp, B::Comp, B::Comp, B::Comp)
where
    B: NumericVector4,
    F: ?Sized + VecFlavor<Backing = B>,
{
    fn from(vec: TypedVector<F>) -> Self {
        vec.to_glam().into()
    }
}

impl<B, F> TypedVectorImpl<F, Dim4>
where
    B: NumericVector4,
    F: ?Sized + VecFlavor<Backing = B>,
{
    pub const X: Self = Self::from_glam(B::X);
    pub const Y: Self = Self::from_glam(B::Y);
    pub const Z: Self = Self::from_glam(B::Z);
    pub const W: Self = Self::from_glam(B::W);

    pub fn new(x: B::Comp, y: B::Comp, z: B::Comp, w: B::Comp) -> Self {
        Self::from_glam(B::new(x, y, z, w))
    }

    pub fn x(&self) -> B::Comp {
        self.as_glam().x()
    }

    pub fn x_mut(&mut self) -> &mut B::Comp {
        self.as_glam_mut().x_mut()
    }

    pub fn y(&self) -> B::Comp {
        self.as_glam().y()
    }

    pub fn y_mut(&mut self) -> &mut B::Comp {
        self.as_glam_mut().y_mut()
    }

    pub fn z(&self) -> B::Comp {
        self.as_glam().z()
    }

    pub fn z_mut(&mut self) -> &mut B::Comp {
        self.as_glam_mut().z_mut()
    }

    pub fn w(&self) -> B::Comp {
        self.as_glam().w()
    }

    pub fn w_mut(&mut self) -> &mut B::Comp {
        self.as_glam_mut().w_mut()
    }
}

impl<B, F> NumericVector4 for TypedVectorImpl<F, Dim4>
where
    B: NumericVector4,
    F: ?Sized + VecFlavor<Backing = B>,
{
    const X: Self = Self::X;
    const Y: Self = Self::Y;
    const Z: Self = Self::Z;
    const W: Self = Self::W;

    fn new(x: Self::Comp, y: Self::Comp, z: Self::Comp, w: Self::Comp) -> Self {
        Self::new(x, y, z, w)
    }

    fn x(&self) -> Self::Comp {
        self.x()
    }

    fn x_mut(&mut self) -> &mut Self::Comp {
        self.x_mut()
    }

    fn y(&self) -> Self::Comp {
        self.y()
    }

    fn y_mut(&mut self) -> &mut Self::Comp {
        self.y_mut()
    }

    fn z(&self) -> Self::Comp {
        self.z()
    }

    fn z_mut(&mut self) -> &mut Self::Comp {
        self.z_mut()
    }

    fn w(&self) -> Self::Comp {
        self.w()
    }

    fn w_mut(&mut self) -> &mut Self::Comp {
        self.w_mut()
    }
}

// SignedNumericVector4
impl<B, F> TypedVectorImpl<F, Dim4>
where
    B: ?Sized + SignedNumericVector4,
    F: ?Sized + VecFlavor<Backing = B>,
{
    pub const NEG_X: Self = Self::from_glam(B::NEG_X);
    pub const NEG_Y: Self = Self::from_glam(B::NEG_Y);
    pub const NEG_Z: Self = Self::from_glam(B::NEG_Z);
    pub const NEG_W: Self = Self::from_glam(B::NEG_W);
}

impl<B, F> SignedNumericVector4 for TypedVector<F>
where
    B: ?Sized + SignedNumericVector4,
    F: ?Sized + VecFlavor<Backing = B>,
{
    const NEG_X: Self = Self::NEG_X;
    const NEG_Y: Self = Self::NEG_Y;
    const NEG_Z: Self = Self::NEG_Z;
    const NEG_W: Self = Self::NEG_W;
}

// FloatingVector2
impl<B, F> TypedVectorImpl<F, Dim2>
where
    B: ?Sized + FloatingVector2,
    F: ?Sized + VecFlavor<Backing = B>,
{
    pub fn from_angle(angle: B::Comp) -> Self {
        Self::from_glam(B::from_angle(angle))
    }

    pub fn angle_between(self, rhs: Self) -> B::Comp {
        self.to_glam().angle_between(rhs.to_glam())
    }

    pub fn perp(self) -> Self {
        self.map_glam(|raw| raw.perp())
    }

    pub fn perp_dot(self, rhs: Self) -> B::Comp {
        self.to_glam().perp_dot(rhs.to_glam())
    }

    pub fn rotate(self, rhs: Self) -> Self {
        self.map_glam(|raw| raw.rotate(rhs.to_glam()))
    }
}

impl<B, F> FloatingVector2 for TypedVector<F>
where
    B: ?Sized + FloatingVector2,
    F: ?Sized + VecFlavor<Backing = B>,
{
    fn from_angle(angle: Self::Comp) -> Self {
        Self::from_angle(angle)
    }

    fn angle_between(self, rhs: Self) -> Self::Comp {
        self.angle_between(rhs)
    }

    fn perp(self) -> Self {
        self.perp()
    }

    fn perp_dot(self, rhs: Self) -> Self::Comp {
        self.perp_dot(rhs)
    }

    fn rotate(self, rhs: Self) -> Self {
        self.rotate(rhs)
    }
}

// FloatingVector3
impl<B, F> TypedVectorImpl<F, Dim3>
where
    B: ?Sized + FloatingVector3,
    F: ?Sized + VecFlavor<Backing = B>,
{
    pub fn angle_between(self, rhs: Self) -> B::Comp {
        self.to_glam().angle_between(rhs.to_glam())
    }

    pub fn any_orthogonal_vector(&self) -> Self {
        Self::from_glam(self.as_glam().any_orthogonal_vector())
    }

    pub fn any_orthonormal_vector(&self) -> Self {
        Self::from_glam(self.as_glam().any_orthonormal_vector())
    }

    pub fn any_orthonormal_pair(&self) -> (Self, Self) {
        let (a, b) = self.as_glam().any_orthonormal_pair();
        (Self::from_glam(a), Self::from_glam(b))
    }
}

impl<B, F> FloatingVector3 for TypedVector<F>
where
    B: ?Sized + FloatingVector3,
    F: ?Sized + VecFlavor<Backing = B>,
{
    fn angle_between(self, rhs: Self) -> Self::Comp {
        self.angle_between(rhs)
    }

    fn any_orthogonal_vector(&self) -> Self {
        self.any_orthogonal_vector()
    }

    fn any_orthonormal_vector(&self) -> Self {
        self.any_orthonormal_vector()
    }

    fn any_orthonormal_pair(&self) -> (Self, Self) {
        self.any_orthonormal_pair()
    }
}

// FloatingVector4
impl<B, F> TypedVectorImpl<F, Dim4>
where
    B: ?Sized + FloatingVector4,
    F: ?Sized + VecFlavor<Backing = B>,
{
}

impl<B, F> FloatingVector4 for TypedVector<F>
where
    B: ?Sized + FloatingVector4,
    F: ?Sized + VecFlavor<Backing = B>,
{
}

// Overload derivations
macro_rules! derive_bin_ops {
	($(
		$trait:ident, $fn:ident
		$(, $trait_assign:ident, $fn_assign:ident)?
		$(for $extra_trait:ident)?
	);*
	$(;)?
	) => {
		$(
			impl<B, F, R> ops::$trait<R> for TypedVector<F>
			where
				B: NumericVector $(+ $extra_trait)?,
				F: ?Sized + VecFlavor<Backing = B> + FlavorCastFrom<R>,
			{
				type Output = Self;

				fn $fn(self, rhs: R) -> Self::Output {
					self.map_glam(|lhs| ops::$trait::$fn(lhs, F::cast_from(rhs).to_glam()))
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
						ops::$trait_assign::$fn_assign(self.as_glam_mut(), F::cast_from(rhs).to_glam());
					}
				}
			)?
		)*
	};
}

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
        self.map_glam(ops::Not::not)
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
        self.map_glam(ops::Neg::neg)
    }
}
