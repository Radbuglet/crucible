use std::borrow::Borrow;
use std::fmt::Debug;
use std::mem::MaybeUninit;

// === Vector Traits === //

// Core

pub trait ValueType: Sized + Debug + Clone + PartialEq {}

impl<T: Debug + Clone + PartialEq> ValueType for T {}

pub trait Vector: ValueType {
	const DIM: usize;
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

	// Extensions
	fn map<F, M>(&self, mut f: F) -> Self
	where
		F: VectorMapFn<Self, M>,
		M: OpVariantMarker,
	{
		Self::new(
			self.comps()
				.enumerate()
				.map(|(index, comp)| f.map(comp, index, self)),
		)
	}

	fn zip<R, F, M>(&self, rhs_vec: &R, mut f: F) -> Self
	where
		R: ?Sized + Vector,
		F: VectorZipFn<Self, R, M>,
		M: OpVariantMarker,
	{
		Self::new(
			self.comps()
				.zip(rhs_vec.comps())
				.enumerate()
				.map(|(index, (lhs, rhs))| f.zip(index, self, lhs, rhs_vec, rhs)),
		)
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
		if self.index < T::DIM {
			let comp = self.vec.comp(self.index);
			self.index += 1;
			Some(comp)
		} else {
			None
		}
	}
}

// Map traits

pub trait OpVariantMarker: sealed::Sealed {}

pub struct OpVariantCompOnly;

pub struct OpVariantCompAndIndex;

pub struct OpVariantCompIndexAndVec;

mod sealed {
	use super::*;

	pub trait Sealed {}

	impl OpVariantMarker for OpVariantCompOnly {}
	impl Sealed for OpVariantCompOnly {}

	impl OpVariantMarker for OpVariantCompAndIndex {}
	impl Sealed for OpVariantCompAndIndex {}

	impl OpVariantMarker for OpVariantCompIndexAndVec {}
	impl Sealed for OpVariantCompIndexAndVec {}
}

pub trait VectorMapFn<V: ?Sized + Vector, M: OpVariantMarker> {
	fn map(&mut self, comp: V::Comp, index: usize, vec: &V) -> V::Comp;
}

impl<F, V> VectorMapFn<V, OpVariantCompOnly> for F
where
	F: FnMut(V::Comp) -> V::Comp,
	V: ?Sized + Vector,
{
	fn map(&mut self, comp: V::Comp, _index: usize, _vec: &V) -> V::Comp {
		(self)(comp)
	}
}

impl<F, V> VectorMapFn<V, OpVariantCompAndIndex> for F
where
	F: FnMut(V::Comp, usize) -> V::Comp,
	V: ?Sized + Vector,
{
	fn map(&mut self, comp: V::Comp, index: usize, _vec: &V) -> V::Comp {
		(self)(comp, index)
	}
}

impl<F, V> VectorMapFn<V, OpVariantCompIndexAndVec> for F
where
	F: FnMut(V::Comp, usize, &V) -> V::Comp,
	V: ?Sized + Vector,
{
	fn map(&mut self, comp: V::Comp, index: usize, vec: &V) -> V::Comp {
		(self)(comp, index, vec)
	}
}

pub trait VectorZipFn<L: ?Sized + Vector, R: Vector, M: OpVariantMarker> {
	fn zip(
		&mut self,
		index: usize,
		left_vec: &L,
		left_comp: L::Comp,
		right_vec: &R,
		right_comp: R::Comp,
	) -> L::Comp;
}

impl<F, L, R> VectorZipFn<L, R, OpVariantCompOnly> for F
where
	F: FnMut(L::Comp, R::Comp) -> L::Comp,
	L: ?Sized + Vector,
	R: ?Sized + Vector,
{
	fn zip(
		&mut self,
		_index: usize,
		_left_vec: &L,
		left_comp: L::Comp,
		_right_vec: &R,
		right_comp: R::Comp,
	) -> L::Comp {
		(self)(left_comp, right_comp)
	}
}

impl<F, L, R> VectorZipFn<L, R, OpVariantCompAndIndex> for F
where
	F: FnMut(usize, L::Comp, R::Comp) -> L::Comp,
	L: ?Sized + Vector,
	R: ?Sized + Vector,
{
	fn zip(
		&mut self,
		index: usize,
		_left_vec: &L,
		left_comp: L::Comp,
		_right_vec: &R,
		right_comp: R::Comp,
	) -> L::Comp {
		(self)(index, left_comp, right_comp)
	}
}

impl<F, L, R> VectorZipFn<L, R, OpVariantCompIndexAndVec> for F
where
	F: FnMut(usize, &L, L::Comp, &R, R::Comp) -> L::Comp,
	L: ?Sized + Vector,
	R: ?Sized + Vector,
{
	fn zip(
		&mut self,
		index: usize,
		left_vec: &L,
		left_comp: L::Comp,
		right_vec: &R,
		right_comp: R::Comp,
	) -> L::Comp {
		(self)(index, left_vec, left_comp, right_vec, right_comp)
	}
}

// === Math traits === //

// Core traits

pub trait Arithmetic<R = Self>: ValueType {
	fn add_wrapping_flag<Q: Borrow<R>>(&self, rhs: Q) -> (Self, bool);

	fn add_wrapping<Q: Borrow<R>>(&self, rhs: Q) -> Self {
		self.add_wrapping_flag(rhs).0
	}

	fn add<Q: Borrow<R>>(&self, rhs: Q) -> Self {
		let (vec, did_wrap) = self.add_wrapping_flag(rhs);
		debug_assert!(!did_wrap);
		vec
	}

	fn try_add<Q: Borrow<R>>(&self, rhs: Q) -> Result<Self, Self> {
		let (vec, did_wrap) = self.add_wrapping_flag(rhs);
		if did_wrap {
			Ok(vec)
		} else {
			Err(vec)
		}
	}

	fn add_saturating<Q: Borrow<R>>(&self, rhs: Q) -> Self;
}

pub trait Invertible: ValueType {
	fn neg(&self) -> Self;
}

pub trait Scalable<R>: ValueType {
	fn mul(&self, rhs: R) -> Self;
	fn div(&self, rhs: R) -> Self;
}

// Vector derivations

pub trait DerivesVectorMath<R: Vector>: Vector {
	type RhsProxy: ValueType;

	fn get_rhs_comp(rhs: &R, index: usize) -> Self::RhsProxy;
}

impl<VL, LC, VR, RC> Arithmetic<VR> for VL
where
	VL: DerivesVectorMath<VR, Comp = LC, RhsProxy = RC>,
	VR: Vector,
	LC: Arithmetic<RC>,
{
	fn add_wrapping_flag<Q: Borrow<VR>>(&self, rhs: Q) -> (Self, bool) {
		let rhs = rhs.borrow();

		let mut did_any_wrap = false;
		let vec = self.map(|lhs: LC, i| {
			let (val, did_wrap) = lhs.add_wrapping_flag(Self::get_rhs_comp(rhs, i));
			if did_wrap {
				did_any_wrap = true;
			}
			val
		});

		(vec, did_any_wrap)
	}

	fn add_wrapping<Q: Borrow<VR>>(&self, rhs: Q) -> Self {
		let rhs = rhs.borrow();
		self.map(|lhs: LC, i| lhs.add_wrapping(Self::get_rhs_comp(rhs, i)))
	}

	fn add<Q: Borrow<VR>>(&self, rhs: Q) -> Self {
		let rhs = rhs.borrow();
		self.map(|lhs: LC, i| lhs.add(Self::get_rhs_comp(rhs, i)))
	}

	fn try_add<Q: Borrow<VR>>(&self, rhs: Q) -> Result<Self, Self> {
		let rhs = rhs.borrow();

		let mut success = true;
		let vec = self.map(|lhs: LC, i| {
			let out = lhs.try_add(Self::get_rhs_comp(rhs, i));
			match out {
				Ok(out) => out,
				Err(out) => {
					success = false;
					out
				}
			}
		});

		if success {
			Ok(vec)
		} else {
			Err(vec)
		}
	}

	fn add_saturating<Q: Borrow<VR>>(&self, rhs: Q) -> Self {
		let rhs = rhs.borrow();
		self.map(|lhs: LC, i| lhs.add_saturating(Self::get_rhs_comp(rhs, i)))
	}
}

impl<V: Vector<Comp = C>, C: Invertible> Invertible for V {
	fn neg(&self) -> Self {
		self.map(|val: C| val.neg())
	}
}

impl<VL, LC, VR, RC> Scalable<VR> for VL
where
	VL: DerivesVectorMath<VR, Comp = LC, RhsProxy = RC>,
	VR: Vector,
	LC: Scalable<RC>,
{
	fn mul(&self, rhs: VR) -> Self {
		let rhs = rhs.borrow();
		self.map(|val: LC, i| val.mul(Self::get_rhs_comp(rhs, i)))
	}

	fn div(&self, rhs: VR) -> Self {
		let rhs = rhs.borrow();
		self.map(|val: LC, i| val.div(Self::get_rhs_comp(rhs, i)))
	}
}

// === Standard Vectors === //

fn decompose_iter<I, T, E, const N: usize>(iter: I) -> Result<[T; N], E>
where
	I: IntoIterator<Item = Result<T, E>>,
{
	let mut array = MaybeUninit::<[T; N]>::uninit();
	let uninit_slice = unsafe { &mut *array.as_mut_ptr().cast::<[MaybeUninit<T>; N]>() };
	let mut i = 0usize;

	for elem in iter {
		if i >= uninit_slice.len() {
			panic!("Too many iterator elements to pack into an array of length {N}.");
		}
		uninit_slice[i].write(elem?);
		i += 1;
	}

	if i < uninit_slice.len() {
		panic!(
			"Expected there to be exactly {} element{} in the iterator but only found {} (missing {})",
			N,
			if N == 1 { "" } else { "s" },
			i,
			N - i,
		);
	}

	Ok(unsafe { array.assume_init() })
}

fn invalid_comp_index_for<T: Vector>(index: usize) -> ! {
	panic!(
		"attempted to get component {index} of a {} dimensional vector.",
		T::DIM
	)
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct Vec3<T> {
	pub x: T,
	pub y: T,
	pub z: T,
}

impl<T: ValueType> Vector for Vec3<T> {
	const DIM: usize = 3;
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
			_ => invalid_comp_index_for::<Self>(index),
		}
	}
}
