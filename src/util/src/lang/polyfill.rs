use std::{
	hash::{self, Hasher},
	ops::{Bound, RangeBounds},
};

use super::std_traits::{OptionLike, ResultLike};

// === Option === //

pub trait OptionPoly: OptionLike {
	fn p_is_some_and<F>(self, f: F) -> bool
	where
		F: FnOnce(Self::Value) -> bool;

	fn p_is_none_or<F>(self, f: F) -> bool
	where
		F: FnOnce(Self::Value) -> bool;
}

impl<T> OptionPoly for Option<T> {
	fn p_is_some_and<F>(self, f: F) -> bool
	where
		F: FnOnce(Self::Value) -> bool,
	{
		self.map_or(false, f)
	}

	fn p_is_none_or<F>(self, f: F) -> bool
	where
		F: FnOnce(Self::Value) -> bool,
	{
		self.map_or(true, f)
	}
}

// === Range === //

pub type AnyRange<T> = (Bound<T>, Bound<T>);

pub trait RangePoly<T: ?Sized>: RangeBounds<T> {
	fn any_range(&self) -> AnyRange<&T> {
		(self.start_bound(), self.end_bound())
	}

	fn any_range_cloned(&self) -> AnyRange<T>
	where
		T: Clone,
	{
		(self.start_bound().cloned(), self.end_bound().cloned())
	}
}

impl<T: ?Sized, I: ?Sized + RangeBounds<T>> RangePoly<T> for I {}

pub trait BoundPoly {
	type Inner;

	fn as_ref(&self) -> Bound<&Self::Inner>;

	fn unwrap_or_unbounded(self) -> Option<Self::Inner>;
}

impl<T> BoundPoly for Bound<T> {
	type Inner = T;

	fn as_ref(&self) -> Bound<&Self::Inner> {
		match self {
			Bound::Included(bound) => Bound::Included(bound),
			Bound::Excluded(bound) => Bound::Excluded(bound),
			Bound::Unbounded => Bound::Unbounded,
		}
	}

	fn unwrap_or_unbounded(self) -> Option<Self::Inner> {
		match self {
			Bound::Included(val) => Some(val),
			Bound::Excluded(val) => Some(val),
			Bound::Unbounded => None,
		}
	}
}

// === Float === //

trait FloatPoly: Sized {
	fn from_frac(num: u32, denom: u32) -> Option<Self>;
}

impl FloatPoly for f32 {
	fn from_frac(num: u32, denom: u32) -> Option<f32> {
		if denom != 0 {
			// Yes, there are truncation errors with this routine. However, none of the routines
			// using this method are dealing with big fractions so this is fine.
			Some((num as f64 / denom as f64) as f32)
		} else {
			None
		}
	}
}

// === Hasher === //

pub trait BuildHasherPoly: hash::BuildHasher {
	fn p_hash_one<H: ?Sized + hash::Hash>(&self, target: &H) -> u64 {
		let mut hasher = self.build_hasher();
		target.hash(&mut hasher);
		hasher.finish()
	}
}

impl<T: ?Sized + hash::BuildHasher> BuildHasherPoly for T {}

// === Result === //

pub trait ResultPoly: ResultLike {
	fn swap_parts(self) -> Result<Self::Error, Self::Success>;
}

impl<T, E> ResultPoly for Result<T, E> {
	fn swap_parts(self) -> Result<Self::Error, Self::Success> {
		match self {
			Ok(ok) => Err(ok),
			Err(err) => Ok(err),
		}
	}
}

pub trait BiResultPoly {
	type Value;

	fn unwrap_either(self) -> Self::Value;
}

impl<T> BiResultPoly for Result<T, T> {
	type Value = T;

	fn unwrap_either(self) -> Self::Value {
		match self {
			Ok(val) => val,
			Err(val) => val,
		}
	}
}

// === Slice === //

pub trait SlicePoly {
	type Elem;

	fn limit_len(&self, max_len: usize) -> &[Self::Elem];
}

impl<T> SlicePoly for [T] {
	type Elem = T;

	fn limit_len(&self, max_len: usize) -> &[Self::Elem] {
		if self.len() > max_len {
			&self[..max_len]
		} else {
			self
		}
	}
}

// === Vec === //

pub trait VecPoly {
	type Elem;

	fn ensure_length_with<F>(&mut self, min_len: usize, f: F)
	where
		F: FnMut() -> Self::Elem;

	fn ensure_slot_with<F>(&mut self, index: usize, f: F) -> &mut Self::Elem
	where
		F: FnMut() -> Self::Elem;
}

impl<T> VecPoly for Vec<T> {
	type Elem = T;

	fn ensure_length_with<F>(&mut self, min_len: usize, f: F)
	where
		F: FnMut() -> Self::Elem,
	{
		if self.len() < min_len {
			self.resize_with(min_len, f);
		}
	}

	fn ensure_slot_with<F>(&mut self, index: usize, f: F) -> &mut Self::Elem
	where
		F: FnMut() -> Self::Elem,
	{
		self.ensure_length_with(index + 1, f);
		&mut self[index]
	}
}
