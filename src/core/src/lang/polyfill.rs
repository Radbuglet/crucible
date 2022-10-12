use std::{
	cell::{Ref, RefMut},
	hash::{self, Hasher},
};

use bytemuck::TransparentWrapper;

use crate::mem::wide_option::WideOption;

use super::{
	lifetime::try_transform,
	std_traits::{OptionLike, ResultLike},
};

// === RefCell === //

pub trait RefPoly<'a>: Sized {
	type Inner;

	fn filter_map_ref<U, F>(me: Self, f: F) -> Result<Ref<'a, U>, Self>
	where
		F: FnOnce(&Self::Inner) -> Option<&U>;
}

impl<'a, T> RefPoly<'a> for Ref<'a, T> {
	type Inner = T;

	fn filter_map_ref<U, F>(me: Self, f: F) -> Result<Ref<'a, U>, Self>
	where
		F: FnOnce(&Self::Inner) -> Option<&U>,
	{
		let backup = Ref::clone(&me);
		let mapped = Ref::map(me, |orig| match f(orig) {
			Some(mapped) => WideOption::some(mapped),
			None => WideOption::none(),
		});

		if mapped.is_some() {
			Ok(Ref::map(mapped, |mapped| mapped.unwrap_ref()))
		} else {
			Err(backup)
		}
	}
}

pub trait RefMutPoly<'a>: Sized {
	type Inner;

	fn filter_map_mut<U, F>(me: Self, f: F) -> Result<RefMut<'a, U>, Self>
	where
		F: FnOnce(&mut Self::Inner) -> Option<&mut U>;
}

impl<'a, T> RefMutPoly<'a> for RefMut<'a, T> {
	type Inner = T;

	fn filter_map_mut<U, F>(me: Self, f: F) -> Result<RefMut<'a, U>, Self>
	where
		F: FnOnce(&mut Self::Inner) -> Option<&mut U>,
	{
		// Utils
		// Thanks to `kpreid` for helping me make the original implementation of this safe.
		trait Either<U, T> {
			fn as_result(&mut self) -> Result<&mut U, &mut T>;
		}

		#[derive(TransparentWrapper)]
		#[repr(transparent)]
		struct Success<U>(U);

		impl<U, T> Either<U, T> for Success<U> {
			fn as_result(&mut self) -> Result<&mut U, &mut T> {
				Ok(&mut self.0)
			}
		}

		#[derive(TransparentWrapper)]
		#[repr(transparent)]
		struct Failure<T>(T);

		impl<U, T> Either<U, T> for Failure<T> {
			fn as_result(&mut self) -> Result<&mut U, &mut T> {
				Err(&mut self.0)
			}
		}

		// Actual implementation
		let mut mapped = RefMut::map(me, |orig| match try_transform(orig, f) {
			Ok(mapped) => Success::wrap_mut(mapped) as &mut dyn Either<U, T>,
			Err(orig) => Failure::wrap_mut(orig) as &mut dyn Either<U, T>,
		});

		match mapped.as_result().is_ok() {
			true => Ok(RefMut::map(mapped, |val| val.as_result().ok().unwrap())),
			false => Err(RefMut::map(mapped, |val| val.as_result().err().unwrap())),
		}
	}
}

// === Option === //

pub trait OptionPoly: OptionLike {
	fn is_some_and<F>(self, f: F) -> bool
	where
		F: FnOnce(Self::Value) -> bool;
}

impl<T> OptionPoly for Option<T> {
	fn is_some_and<F>(self, f: F) -> bool
	where
		F: FnOnce(Self::Value) -> bool,
	{
		self.map_or(false, f)
	}
}

// === Range === //

use core::ops::Bound;
use std::ops::RangeBounds;

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
			// using this object are dealing with big fractions so this is fine.
			Some((num as f64 / denom as f64) as f32)
		} else {
			None
		}
	}
}

// === Hasher === //

pub trait BuildHasherPoly: hash::BuildHasher {
	fn hash_one<H: ?Sized + hash::Hash>(&self, target: &H) -> u64 {
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
