#![allow(unconditional_recursion)] // TODO: Remove

use std::{
    fmt::{Debug, Display},
    hash::Hash,
    iter::{Product, Sum},
    ops::{
        Add, AddAssign, BitAnd, BitOr, BitXor, Div, DivAssign, Index, IndexMut, Mul, MulAssign,
        Neg, Not, Rem, RemAssign, Shl, Shr, Sub, SubAssign,
    },
};

use num_traits::Num;
use crucible_utils::traits::ArrayLike;

// === Dim class === //

pub trait DimClass: Sized + 'static {
    const DIM: usize;
}

#[non_exhaustive]
pub struct Dim2;

impl DimClass for Dim2 {
    const DIM: usize = 2;
}

#[non_exhaustive]
pub struct Dim3;

impl DimClass for Dim3 {
    const DIM: usize = 3;
}

#[non_exhaustive]
pub struct Dim4;

impl DimClass for Dim4 {
    const DIM: usize = 4;
}

// === Definitions === //

pub trait GlamBacked: Sized {
    type Glam;

    fn to_glam(self) -> Self::Glam;

    fn as_glam(&self) -> &Self::Glam;

    fn as_glam_mut(&mut self) -> &mut Self::Glam;

    fn from_glam(glam: Self::Glam) -> Self;

    fn from_glam_ref(glam: &Self::Glam) -> &Self;

    fn from_glam_mut(glam: &mut Self::Glam) -> &mut Self;

    // === Derived === //

    // N.B. Default `impl`s are copied verbatim into `TypedGlam`'s inherent impls to make them `const`
    // safe. If you change things here, you should probably change them there as well.

    fn map_glam<R, F>(self, f: F) -> R
    where
        R: GlamBacked,
        F: FnOnce(Self::Glam) -> R::Glam,
    {
        R::from_glam(f(self.to_glam()))
    }

    fn cast_glam<T: GlamBacked<Glam = Self::Glam>>(self) -> T {
        T::from_glam(self.to_glam())
    }

    fn cast_glam_ref<T: GlamBacked<Glam = Self::Glam>>(&self) -> &T {
        T::from_glam_ref(self.as_glam())
    }

    fn cast_glam_mut<T: GlamBacked<Glam = Self::Glam>>(&mut self) -> &mut T {
        T::from_glam_mut(self.as_glam_mut())
    }
}

pub trait CastVecFrom<C> {
    fn cast_from(other: C) -> Self;
}

pub trait NumericVector:
    'static
    + Debug
    + Display
    + Copy
    + PartialEq
    + Default
    + Add<Output = Self>
    + AddAssign
    + Sub<Output = Self>
    + SubAssign
    + Mul<Output = Self>
    + MulAssign
    + Div<Output = Self>
    + DivAssign
    + Rem<Output = Self>
    + RemAssign
    + Index<usize, Output = Self::Comp>
    + IndexMut<usize>
    + for<'a> Sum<&'a Self>
    + for<'a> Product<&'a Self>
    + GlamBacked
    + CastVecFrom<Self>
{
    // Types
    type Dim: DimClass;
    type Comp: Debug + Display + Copy + Num; // TODO: Narrow these bounds a bit more.
    type CompArray: ArrayLike<Elem = Self::Comp>;
    type Mask;

    // Constants
    const ZERO: Self;
    const ONE: Self;

    fn unit_axis(index: usize) -> Self;

    // Constructors
    fn from_array(a: Self::CompArray) -> Self;
    fn to_array(&self) -> Self::CompArray;
    fn from_slice(slice: &[Self::Comp]) -> Self;
    fn write_to_slice(self, slice: &mut [Self::Comp]);
    fn splat(v: Self::Comp) -> Self;

    // Component-wise logical manipulations
    fn select(mask: Self::Mask, if_true: Self, if_false: Self) -> Self;
    fn min(self, rhs: Self) -> Self;
    fn max(self, rhs: Self) -> Self;
    fn clamp(self, min: Self, max: Self) -> Self;
    fn min_element(self) -> Self::Comp;
    fn max_element(self) -> Self::Comp;

    fn cmpeq(self, rhs: Self) -> Self::Mask;
    fn cmpne(self, rhs: Self) -> Self::Mask;
    fn cmpge(self, rhs: Self) -> Self::Mask;
    fn cmpgt(self, rhs: Self) -> Self::Mask;
    fn cmple(self, rhs: Self) -> Self::Mask;
    fn cmplt(self, rhs: Self) -> Self::Mask;

    // Woo! Inner products!
    fn dot(self, rhs: Self) -> Self::Comp;
    fn dot_into_vec(self, rhs: Self) -> Self;

    // Typed-glam extensions
    fn cast<V: CastVecFrom<Self>>(self) -> V {
        V::cast_from(self)
    }
}

pub trait IntegerVector:
    NumericVector
    + Eq
    + Hash
    + BitAnd<Output = Self>
    + BitOr<Output = Self>
    + BitXor<Output = Self>
    + Shl<Output = Self>
    + Shr<Output = Self>
    + Not<Output = Self>
{
}

pub trait SignedVector: NumericVector + Neg<Output = Self> {
    const NEG_ONE: Self;

    fn abs(self) -> Self;
    fn signum(self) -> Self;
    fn is_negative_bitmask(self) -> u32;
    fn copysign(self, rhs: Self) -> Self;
}

pub trait FloatingVector: SignedVector {
    const NAN: Self;

    fn is_finite(self) -> bool;
    fn is_nan(self) -> bool;
    fn is_nan_mask(self) -> Self::Mask;
    fn length(self) -> Self::Comp;
    fn length_squared(self) -> Self::Comp;
    fn length_recip(self) -> Self::Comp;
    fn distance(self, rhs: Self) -> Self::Comp;
    fn distance_squared(self, rhs: Self) -> Self::Comp;
    fn normalize(self) -> Self;
    fn try_normalize(self) -> Option<Self>;
    fn normalize_or_zero(self) -> Self;
    fn is_normalized(self) -> bool;
    fn project_onto(self, rhs: Self) -> Self;
    fn reject_from(self, rhs: Self) -> Self;
    fn project_onto_normalized(self, rhs: Self) -> Self;
    fn reject_from_normalized(self, rhs: Self) -> Self;
    fn round(self) -> Self;
    fn floor(self) -> Self;
    fn ceil(self) -> Self;
    fn fract(self) -> Self;
    fn exp(self) -> Self;
    fn powf(self, n: Self::Comp) -> Self;
    fn recip(self) -> Self;
    fn lerp(self, rhs: Self, s: Self::Comp) -> Self;
    fn abs_diff_eq(self, rhs: Self, max_abs_diff: Self::Comp) -> bool;
    fn clamp_length(self, min: Self::Comp, max: Self::Comp) -> Self;
    fn clamp_length_max(self, max: Self::Comp) -> Self;
    fn clamp_length_min(self, min: Self::Comp) -> Self;
    fn mul_add(self, a: Self, b: Self) -> Self;
}

pub trait NumericVector2:
    NumericVector<Dim = Dim2> + From<(Self::Comp, Self::Comp)> + Into<(Self::Comp, Self::Comp)>
{
    const X: Self;
    const Y: Self;

    fn new(x: Self::Comp, y: Self::Comp) -> Self;

    fn x(&self) -> Self::Comp;
    fn x_mut(&mut self) -> &mut Self::Comp;

    fn y(&self) -> Self::Comp;
    fn y_mut(&mut self) -> &mut Self::Comp;
}

pub trait SignedNumericVector2: NumericVector2 + SignedVector {
    const NEG_X: Self;
    const NEG_Y: Self;
}

pub trait NumericVector3:
    NumericVector<Dim = Dim3>
    + From<(Self::Comp, Self::Comp, Self::Comp)>
    + Into<(Self::Comp, Self::Comp, Self::Comp)>
{
    const X: Self;
    const Y: Self;
    const Z: Self;

    fn new(x: Self::Comp, y: Self::Comp, z: Self::Comp) -> Self;
    fn cross(self, rhs: Self) -> Self;

    fn x(&self) -> Self::Comp;
    fn x_mut(&mut self) -> &mut Self::Comp;

    fn y(&self) -> Self::Comp;
    fn y_mut(&mut self) -> &mut Self::Comp;

    fn z(&self) -> Self::Comp;
    fn z_mut(&mut self) -> &mut Self::Comp;
}

pub trait SignedNumericVector3: NumericVector3 + SignedVector {
    const NEG_X: Self;
    const NEG_Y: Self;
    const NEG_Z: Self;
}

pub trait NumericVector4:
    NumericVector<Dim = Dim4>
    + From<(Self::Comp, Self::Comp, Self::Comp, Self::Comp)>
    + Into<(Self::Comp, Self::Comp, Self::Comp, Self::Comp)>
{
    const X: Self;
    const Y: Self;
    const Z: Self;
    const W: Self;

    fn new(x: Self::Comp, y: Self::Comp, z: Self::Comp, w: Self::Comp) -> Self;

    fn x(&self) -> Self::Comp;
    fn x_mut(&mut self) -> &mut Self::Comp;

    fn y(&self) -> Self::Comp;
    fn y_mut(&mut self) -> &mut Self::Comp;

    fn z(&self) -> Self::Comp;
    fn z_mut(&mut self) -> &mut Self::Comp;

    fn w(&self) -> Self::Comp;
    fn w_mut(&mut self) -> &mut Self::Comp;
}

pub trait SignedNumericVector4: NumericVector4 + SignedVector {
    const NEG_X: Self;
    const NEG_Y: Self;
    const NEG_Z: Self;
    const NEG_W: Self;
}

pub trait FloatingVector2: FloatingVector + SignedNumericVector2 {
    fn from_angle(angle: Self::Comp) -> Self;
    fn angle_between(self, rhs: Self) -> Self::Comp;
    fn perp(self) -> Self;
    fn perp_dot(self, rhs: Self) -> Self::Comp;
    fn rotate(self, rhs: Self) -> Self;
}

pub trait FloatingVector3: FloatingVector + SignedNumericVector3 {
    fn angle_between(self, rhs: Self) -> Self::Comp;
    fn any_orthogonal_vector(&self) -> Self;
    fn any_orthonormal_vector(&self) -> Self;
    fn any_orthonormal_pair(&self) -> (Self, Self);
}

pub trait FloatingVector4: FloatingVector + SignedNumericVector4 {}

// === Implementations === //

macro_rules! impl_glam_convert_identity {
	($($ty:ty),*$(,)?) => {$(
		impl GlamBacked for $ty {
			type Glam = Self;

			fn to_glam(self) -> Self::Glam {
				self
			}

			fn as_glam(&self) -> &Self::Glam {
				self
			}

			fn as_glam_mut(&mut self) -> &mut Self::Glam {
				self
			}

			fn from_glam(glam: Self::Glam) -> Self {
				glam
			}

			fn from_glam_ref(glam: &Self::Glam) -> &Self {
				glam
			}

			fn from_glam_mut(glam: &mut Self::Glam) -> &mut Self {
				glam
			}
		}
	)*};
}

impl_glam_convert_identity!(
    glam::Vec2,
    glam::Vec3,
    glam::Vec3A,
    glam::Vec4,
    glam::DVec2,
    glam::DVec3,
    glam::DVec4,
    glam::IVec2,
    glam::IVec3,
    glam::IVec4,
    glam::UVec2,
    glam::UVec3,
    glam::UVec4,
);

macro_rules! numeric_vector_forwards {
    () => {
        const ZERO: Self = Self::ZERO;
        const ONE: Self = Self::ONE;

        fn from_array(a: Self::CompArray) -> Self {
            Self::from_array(a)
        }

        fn to_array(&self) -> Self::CompArray {
            self.to_array()
        }

        fn from_slice(slice: &[Self::Comp]) -> Self {
            Self::from_slice(slice)
        }

        fn write_to_slice(self, slice: &mut [Self::Comp]) {
            self.write_to_slice(slice)
        }

        fn splat(v: Self::Comp) -> Self {
            Self::splat(v)
        }

        fn select(mask: Self::Mask, if_true: Self, if_false: Self) -> Self {
            Self::select(mask, if_true, if_false)
        }

        fn min(self, rhs: Self) -> Self {
            self.min(rhs)
        }

        fn max(self, rhs: Self) -> Self {
            self.max(rhs)
        }

        fn clamp(self, min: Self, max: Self) -> Self {
            self.clamp(min, max)
        }

        fn min_element(self) -> Self::Comp {
            self.min_element()
        }

        fn max_element(self) -> Self::Comp {
            self.max_element()
        }

        fn cmpeq(self, rhs: Self) -> Self::Mask {
            self.cmpeq(rhs)
        }

        fn cmpne(self, rhs: Self) -> Self::Mask {
            self.cmpne(rhs)
        }

        fn cmpge(self, rhs: Self) -> Self::Mask {
            self.cmpge(rhs)
        }

        fn cmpgt(self, rhs: Self) -> Self::Mask {
            self.cmpgt(rhs)
        }

        fn cmple(self, rhs: Self) -> Self::Mask {
            self.cmple(rhs)
        }

        fn cmplt(self, rhs: Self) -> Self::Mask {
            self.cmplt(rhs)
        }

        fn dot(self, rhs: Self) -> Self::Comp {
            self.dot(rhs)
        }

        fn dot_into_vec(self, rhs: Self) -> Self {
            self.dot_into_vec(rhs)
        }
    };
}

pub(crate) use numeric_vector_forwards;

macro_rules! impl_numeric_vector {
	(
		$($ty:ty, $bool_ty:ty, $comp:ty, $dim:ty);
		*$(;)?
	) => {
		$(
			impl CastVecFrom<$ty> for $ty {
				fn cast_from(other: $ty) -> Self {
					other
				}
			}

			impl NumericVector for $ty {
				type Dim = $dim;
				type Comp = $comp;
				type CompArray = [Self::Comp; <$dim as DimClass>::DIM];
				type Mask = $bool_ty;

				fn unit_axis(index: usize) -> Self {
					<$ty>::AXES[index]
				}

				numeric_vector_forwards!();
			}
		)*
	};
}

impl_numeric_vector!(
    glam::Vec2,  glam::BVec2,  f32,  Dim2;
    glam::Vec3,  glam::BVec3,  f32,  Dim3;
    glam::Vec3A, glam::BVec3A, f32,  Dim3;
    glam::DVec2, glam::BVec2,  f64,  Dim2;
    glam::DVec3, glam::BVec3,  f64,  Dim3;
    glam::DVec4, glam::BVec4,  f64,  Dim4;
    glam::IVec2, glam::BVec2,  i32,  Dim2;
    glam::IVec3, glam::BVec3,  i32,  Dim3;
    glam::IVec4, glam::BVec4,  i32,  Dim4;
    glam::UVec2, glam::BVec2,  u32,  Dim2;
    glam::UVec3, glam::BVec3,  u32,  Dim3;
    glam::UVec4, glam::BVec4,  u32,  Dim4;
);

// N.B. Unfortunately, the type of Vec4's boolean vector changes between scalar and SIMD
// implementations.
#[cfg(not(any(
    feature = "core-simd",
    target_feature = "sse2",
    target_feature = "simd128"
)))]
impl_numeric_vector!(glam::Vec4, glam::BVec4, f32, Dim4);

#[cfg(all(
    target_feature = "sse2",
    not(any(feature = "core-simd", feature = "scalar-math"))
))]
impl_numeric_vector!(glam::Vec4, glam::BVec4A, f32, Dim4);

macro_rules! impl_integer_vector {
	($($ty:ty),*$(,)?) => {$(
		impl IntegerVector for $ty {}
	)*};
}

impl_integer_vector!(
    glam::IVec2,
    glam::IVec3,
    glam::IVec4,
    glam::UVec2,
    glam::UVec3,
    glam::UVec4,
);

macro_rules! signed_vector_forwards {
    () => {
        const NEG_ONE: Self = Self::NEG_ONE;

        fn abs(self) -> Self {
            self.abs()
        }

        fn signum(self) -> Self {
            self.signum()
        }

        fn is_negative_bitmask(self) -> u32 {
            self.is_negative_bitmask()
        }

        fn copysign(self, rhs: Self) -> Self {
            self.copysign(rhs)
        }
    };
}

pub(crate) use signed_vector_forwards;

macro_rules! impl_signed_vector {
	($($ty:ty),*$(,)?) => {$(
		impl SignedVector for $ty {
			signed_vector_forwards!();
		}
	)*};
}

impl_signed_vector!(
    glam::Vec2,
    glam::Vec3,
    glam::Vec3A,
    glam::Vec4,
    glam::DVec2,
    glam::DVec3,
    glam::DVec4,
    glam::IVec2,
    glam::IVec3,
    glam::IVec4,
);

macro_rules! floating_vector_forwards {
    () => {
        const NAN: Self = Self::NAN;

        fn is_finite(self) -> bool {
            self.is_finite()
        }

        fn is_nan(self) -> bool {
            self.is_nan()
        }

        fn is_nan_mask(self) -> Self::Mask {
            self.is_nan_mask()
        }

        fn length(self) -> Self::Comp {
            self.length()
        }

        fn length_squared(self) -> Self::Comp {
            self.length_squared()
        }

        fn length_recip(self) -> Self::Comp {
            self.length_recip()
        }

        fn distance(self, rhs: Self) -> Self::Comp {
            self.distance(rhs)
        }

        fn distance_squared(self, rhs: Self) -> Self::Comp {
            self.distance_squared(rhs)
        }

        fn normalize(self) -> Self {
            self.normalize()
        }

        fn try_normalize(self) -> Option<Self> {
            self.try_normalize()
        }

        fn normalize_or_zero(self) -> Self {
            self.normalize_or_zero()
        }

        fn is_normalized(self) -> bool {
            self.is_normalized()
        }

        fn project_onto(self, rhs: Self) -> Self {
            self.project_onto(rhs)
        }

        fn reject_from(self, rhs: Self) -> Self {
            self.reject_from(rhs)
        }

        fn project_onto_normalized(self, rhs: Self) -> Self {
            self.project_onto_normalized(rhs)
        }

        fn reject_from_normalized(self, rhs: Self) -> Self {
            self.reject_from_normalized(rhs)
        }

        fn round(self) -> Self {
            self.round()
        }

        fn floor(self) -> Self {
            self.floor()
        }

        fn ceil(self) -> Self {
            self.ceil()
        }

        fn fract(self) -> Self {
            self.fract()
        }

        fn exp(self) -> Self {
            self.exp()
        }

        fn powf(self, n: Self::Comp) -> Self {
            self.powf(n)
        }

        fn recip(self) -> Self {
            self.recip()
        }

        fn lerp(self, rhs: Self, s: Self::Comp) -> Self {
            self.lerp(rhs, s)
        }

        fn abs_diff_eq(self, rhs: Self, max_abs_diff: Self::Comp) -> bool {
            self.abs_diff_eq(rhs, max_abs_diff)
        }

        fn clamp_length(self, min: Self::Comp, max: Self::Comp) -> Self {
            self.clamp_length(min, max)
        }

        fn clamp_length_max(self, max: Self::Comp) -> Self {
            self.clamp_length_max(max)
        }

        fn clamp_length_min(self, min: Self::Comp) -> Self {
            self.clamp_length_min(min)
        }

        fn mul_add(self, a: Self, b: Self) -> Self {
            self.mul_add(a, b)
        }
    };
}

pub(crate) use floating_vector_forwards;

macro_rules! impl_floating_vector {
	($($ty:ty),*$(,)?) => {$(
		impl FloatingVector for $ty {
			floating_vector_forwards!();
		}
	)*};
}

impl_floating_vector!(
    glam::Vec2,
    glam::Vec3,
    glam::Vec3A,
    glam::Vec4,
    glam::DVec2,
    glam::DVec3,
    glam::DVec4,
);

macro_rules! impl_numeric_vector_2 {
	($($ty:ty),*$(,)?) => {$(
		impl NumericVector2 for $ty {
			const X: Self = Self::X;
			const Y: Self = Self::Y;

			fn new(x: Self::Comp, y: Self::Comp) -> Self {
				Self::new(x, y)
			}

			fn x(&self) -> Self::Comp {
				self.x
			}

			fn x_mut(&mut self) -> &mut Self::Comp {
				&mut self.x
			}

			fn y(&self) -> Self::Comp {
				self.y
			}

			fn y_mut(&mut self) -> &mut Self::Comp {
				&mut self.y
			}
		}
	)*};
}

impl_numeric_vector_2!(glam::Vec2, glam::DVec2, glam::IVec2, glam::UVec2);

macro_rules! impl_signed_numeric_vector_2 {
	($($ty:ty),*$(,)?) => {$(
		impl SignedNumericVector2 for $ty {
			const NEG_X: Self = Self::NEG_X;
			const NEG_Y: Self = Self::NEG_Y;
		}
	)*};
}

impl_signed_numeric_vector_2!(glam::Vec2, glam::DVec2, glam::IVec2);

macro_rules! impl_numeric_vector_3 {
	($($ty:ty),*$(,)?) => {$(
		impl NumericVector3 for $ty {
			const X: Self = Self::X;
			const Y: Self = Self::Y;
			const Z: Self = Self::Z;

			fn new(x: Self::Comp, y: Self::Comp, z: Self::Comp) -> Self {
				Self::new(x, y, z)
			}

			fn cross(self, rhs: Self) -> Self {
				self.cross(rhs)
			}

			fn x(&self) -> Self::Comp {
				self.x
			}

			fn x_mut(&mut self) -> &mut Self::Comp {
				&mut self.x
			}

			fn y(&self) -> Self::Comp {
				self.y
			}

			fn y_mut(&mut self) -> &mut Self::Comp {
				&mut self.y
			}

			fn z(&self) -> Self::Comp {
				self.z
			}

			fn z_mut(&mut self) -> &mut Self::Comp {
				&mut self.z
			}
		}
	)*};
}

impl_numeric_vector_3!(
    glam::Vec3,
    glam::Vec3A,
    glam::DVec3,
    glam::IVec3,
    glam::UVec3,
);

macro_rules! impl_signed_numeric_vector_3 {
	($($ty:ty),*$(,)?) => {$(
		impl SignedNumericVector3 for $ty {
			const NEG_X: Self = Self::NEG_X;
			const NEG_Y: Self = Self::NEG_Y;
			const NEG_Z: Self = Self::NEG_Z;
		}
	)*};
}

impl_signed_numeric_vector_3!(glam::Vec3, glam::Vec3A, glam::DVec3, glam::IVec3);

macro_rules! impl_numeric_vector_4 {
	($($ty:ty),*$(,)?) => {$(
		impl NumericVector4 for $ty {
			const X: Self = Self::X;
			const Y: Self = Self::Y;
			const Z: Self = Self::Z;
			const W: Self = Self::W;

			fn new(x: Self::Comp, y: Self::Comp, z: Self::Comp, w: Self::Comp) -> Self {
				Self::new(x, y, z, w)
			}

			fn x(&self) -> Self::Comp {
				self.x
			}

			fn x_mut(&mut self) -> &mut Self::Comp {
				&mut self.x
			}

			fn y(&self) -> Self::Comp {
				self.y
			}

			fn y_mut(&mut self) -> &mut Self::Comp {
				&mut self.y
			}

			fn z(&self) -> Self::Comp {
				self.z
			}

			fn z_mut(&mut self) -> &mut Self::Comp {
				&mut self.z
			}

			fn w(&self) -> Self::Comp {
				self.w
			}

			fn w_mut(&mut self) -> &mut Self::Comp {
				&mut self.w
			}
		}
	)*};
}

impl_numeric_vector_4!(glam::Vec4, glam::DVec4, glam::IVec4, glam::UVec4);

macro_rules! impl_signed_numeric_vector_4 {
	($($ty:ty),*$(,)?) => {$(
		impl SignedNumericVector4 for $ty {
			const NEG_X: Self = Self::NEG_X;
			const NEG_Y: Self = Self::NEG_Y;
			const NEG_Z: Self = Self::NEG_Z;
			const NEG_W: Self = Self::NEG_W;
		}
	)*};
}

impl_signed_numeric_vector_4!(glam::Vec4, glam::DVec4, glam::IVec4);

macro_rules! impl_floating_vector_2 {
	($($ty:ty),*$(,)?) => {$(
		impl FloatingVector2 for $ty {
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
	)*};
}

impl_floating_vector_2!(glam::Vec2, glam::DVec2);

macro_rules! impl_floating_vector_3 {
	($($ty:ty),*$(,)?) => {$(
		impl FloatingVector3 for $ty {
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
	)*};
}

impl_floating_vector_3!(glam::Vec3, glam::Vec3A, glam::DVec3);

macro_rules! impl_floating_vector_4 {
	($($ty:ty),*$(,)?) => {$(
		impl FloatingVector4 for $ty {}
	)*};
}

impl_floating_vector_4!(glam::Vec4, glam::DVec4);
