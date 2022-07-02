use std::borrow::Borrow;
use std::fmt::Debug;

use crate::polyfill::iter::decompose_iter;
use crate::polyfill::result::unwrap_either;

// === Vector Traits === //

// Dimension classes

pub trait DimClassOf<V: Vector>: DimClassSupports<V, Self> {
	fn dim_of(vec: &V) -> usize;
}

pub trait DimClassSupports<V: Vector, D: ?Sized + DimClassOf<V>> {}

pub struct Dim3(!);

impl<V: Vector> DimClassOf<V> for Dim3 {
	fn dim_of(_: &V) -> usize {
		3
	}
}

impl<V: Vector> DimClassSupports<V, Dim3> for Dim3 {}

// Core

pub trait ValueType: Sized + Debug + Clone + PartialEq {}

impl<T: Debug + Clone + PartialEq> ValueType for T {}

pub trait Vector: ValueType {
	type Dim: DimClassOf<Self>;
	type Comp: ValueType;

	// (De)composition
	fn new_prop_err<I, E>(iter: I) -> Result<Self, E>
	where
		I: Iterator<Item = Result<Self::Comp, E>>;

	fn try_new<I, C: TryInto<Self::Comp>>(iter: I) -> Result<Self, C::Error>
	where
		I: IntoIterator<Item = C>,
	{
		Self::new_prop_err(iter.into_iter().map(TryInto::try_into))
	}

	fn new<I, C: Into<Self::Comp>>(iter: I) -> Self
	where
		I: IntoIterator<Item = C>,
	{
		Self::try_new(iter).unwrap()
	}

	fn comp(&self, index: usize) -> Self::Comp;

	fn comps(&self) -> VecCompIter<'_, Self> {
		VecCompIter::new(self)
	}

	fn dim(&self) -> usize {
		<Self::Dim as DimClassOf<Self>>::dim_of(self)
	}

	// Extensions
	fn map_enumerated<F>(&self, mut f: F) -> Self
	where
		F: FnMut(Self::Comp, usize, &Self) -> Self::Comp,
	{
		Self::new(
			self.comps()
				.enumerate()
				.map(|(index, comp)| f(comp, index, self)),
		)
	}

	fn map<F>(&self, mut f: F) -> Self
	where
		F: FnMut(Self::Comp) -> Self::Comp,
	{
		self.map_enumerated(|v, _, _| f(v))
	}

	fn zip_enumerated<R, F>(&self, rhs_vec: &R, mut f: F) -> Self
	where
		R: ?Sized + Vector,
		F: FnMut(Self::Comp, R::Comp, usize, &Self, &R) -> Self::Comp,
		Self::Dim: DimClassSupports<R, R::Dim>,
	{
		Self::new(
			self.comps()
				.zip(rhs_vec.comps())
				.enumerate()
				.map(|(index, (lhs, rhs))| f(lhs, rhs, index, self, rhs_vec)),
		)
	}

	fn zip<R, F>(&self, rhs_vec: &R, mut f: F) -> Self
	where
		R: ?Sized + Vector,
		F: FnMut(Self::Comp, R::Comp) -> Self::Comp,
		Self::Dim: DimClassSupports<R, R::Dim>,
	{
		self.zip_enumerated(rhs_vec, |a, b, _, _, _| f(a, b))
	}
}

#[derive(Debug, Clone)]
pub struct VecCompIter<'a, T: ?Sized> {
	vec: &'a T,
	index: usize,
}

impl<'a, T: ?Sized> VecCompIter<'a, T> {
	pub fn new(target: &'a T) -> Self {
		Self {
			vec: target,
			index: 0,
		}
	}
}

impl<T: ?Sized + Vector> Iterator for VecCompIter<'_, T> {
	type Item = T::Comp;

	fn next(&mut self) -> Option<Self::Item> {
		if self.index < self.vec.dim() {
			let comp = self.vec.comp(self.index);
			self.index += 1;
			Some(comp)
		} else {
			None
		}
	}
}

// === Math traits === //

// Core traits

pub trait Arithmetic<R = Self>: ValueType {
	fn try_add<Q: Borrow<R>>(&self, rhs: Q) -> Result<Self, Self>;

	fn add_wrapping<Q: Borrow<R>>(&self, rhs: Q) -> Self {
		unwrap_either(self.try_add(rhs))
	}

	fn add<Q: Borrow<R>>(&self, rhs: Q) -> Self {
		let vec = self.try_add(rhs);
		debug_assert!(vec.is_ok());
		unwrap_either(vec)
	}

	fn add_saturating<Q: Borrow<R>>(&self, rhs: Q) -> Self;
}

// Standard implementations

// macro impl_for_math_primitives($($ty:ty),*) {
// 	impl Arithmetic for $ty {
// 		fn try_add<Q: Borrow<R>>(&self, rhs: Q) -> Result<Self, Self> {
// 			match self.checked_add(*rhs.borrow()) {
// 				Some(value) => Ok(value),
// 				None => self.wrapping_add(*rhs.borrow()),
// 			}
// 		}
//
// 		fn add_wrapping<Q: Borrow<R>>(&self, rhs: Q) -> Self {
// 			self.wrapping_add(*rhs.borrow())
// 		}
//
// 		fn add<Q: Borrow<R>>(&self, rhs: Q) -> Self {
// 			*self + *rhs.borrow()
// 		}
//
// 		fn add_saturating<Q: Borrow<R>>(&self, rhs: Q) -> Self {
// 			self.saturating_add(*rhs.borrow())
// 		}
// 	}
// }

// Vector derivations

pub trait DerivesVectorMath<R: Vector>: Vector {
	type RhsProxy: ValueType;

	fn get_rhs_comp(rhs: &R, index: usize) -> Self::RhsProxy;
}

impl<VL, CL, VR, CR> Arithmetic<VR> for VL
where
	VL: DerivesVectorMath<VR, Comp = CL, RhsProxy = CR>,
	VR: Vector,
	CL: Arithmetic<CR>,
{
	fn try_add<Q: Borrow<VR>>(&self, rhs: Q) -> Result<Self, Self> {
		let rhs = rhs.borrow();

		let mut success = true;
		let vec = self.map_enumerated(|lhs, i, _| {
			let out = lhs.try_add(Self::get_rhs_comp(rhs, i));
			if out.is_err() {
				success = false;
			}
			unwrap_either(out)
		});

		match success {
			true => Ok(vec),
			false => Err(vec),
		}
	}

	fn add_wrapping<Q: Borrow<VR>>(&self, rhs: Q) -> Self {
		let rhs = rhs.borrow();
		self.map_enumerated(|lhs, i, _| lhs.add_wrapping(Self::get_rhs_comp(rhs, i)))
	}

	fn add<Q: Borrow<VR>>(&self, rhs: Q) -> Self {
		let rhs = rhs.borrow();
		self.map_enumerated(|lhs, i, _| lhs.add(Self::get_rhs_comp(rhs, i)))
	}

	fn add_saturating<Q: Borrow<VR>>(&self, rhs: Q) -> Self {
		let rhs = rhs.borrow();
		self.map_enumerated(|lhs, i, _| lhs.add_saturating(Self::get_rhs_comp(rhs, i)))
	}
}

// === Standard Vectors === //

fn invalid_comp_index_for<T: Vector>(vec: &T, index: usize) -> ! {
	panic!(
		"attempted to get component {index} of a {} dimensional vector.",
		vec.dim(),
	)
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct Vec3<T> {
	pub x: T,
	pub y: T,
	pub z: T,
}

impl<T: ValueType> Vector for Vec3<T> {
	type Dim = Dim3;
	type Comp = T;

	fn new_prop_err<I, E>(iter: I) -> Result<Self, E>
	where
		I: Iterator<Item = Result<Self::Comp, E>>,
	{
		let [x, y, z] = decompose_iter(iter)?;
		Ok(Self { x, y, z })
	}

	fn comp(&self, index: usize) -> Self::Comp {
		match index {
			0 => self.x.clone(),
			1 => self.y.clone(),
			2 => self.z.clone(),
			_ => invalid_comp_index_for(self, index),
		}
	}

	fn dim(&self) -> usize {
		3
	}
}
