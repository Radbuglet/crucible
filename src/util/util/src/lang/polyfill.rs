use std::{
	hash::{self, Hasher},
	ops::{Bound, RangeBounds},
};

use super::std_traits::OptionLike;

// === Option === //

pub trait OptionPoly: OptionLike {
	#[allow(clippy::wrong_self_convention)] // (this follows the standard library's conventions)
	fn is_none_or<F>(self, f: F) -> bool
	where
		F: FnOnce(Self::Value) -> bool;
}

impl<T> OptionPoly for Option<T> {
	fn is_none_or<F>(self, f: F) -> bool
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

// === Hasher === //

pub trait BuildHasherPoly: hash::BuildHasher {
	fn p_hash_one<H: ?Sized + hash::Hash>(&self, target: &H) -> u64 {
		let mut hasher = self.build_hasher();
		target.hash(&mut hasher);
		hasher.finish()
	}
}

impl<T: ?Sized + hash::BuildHasher> BuildHasherPoly for T {}
