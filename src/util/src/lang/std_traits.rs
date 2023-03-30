use std::{
	borrow::BorrowMut,
	cmp::Ordering,
	ops::{Index, IndexMut},
};

use crate::mem::array::arr_from_iter;

// === OptionLike === //

pub trait OptionLike: Sized {
	type Value;

	fn raw_option(self) -> Option<Self::Value>;
}

impl<T> OptionLike for Option<T> {
	type Value = T;

	fn raw_option(self) -> Option<Self::Value> {
		self
	}
}

impl<T, E> OptionLike for Result<T, E> {
	type Value = T;

	fn raw_option(self) -> Option<Self::Value> {
		self.ok()
	}
}

// === ResultLike === //

pub trait ResultLike: Sized {
	type Success;
	type Error;

	fn raw_result(self) -> Result<Self::Success, Self::Error>;
}

impl<T, E> ResultLike for Result<T, E> {
	type Success = T;
	type Error = E;

	fn raw_result(self) -> Result<Self::Success, Self::Error> {
		self
	}
}

// === SliceLike === //

pub trait SliceLike:
	BorrowMut<[Self::Elem]> + AsMut<[Self::Elem]> + Index<usize, Output = Self::Elem> + IndexMut<usize>
{
	type Elem;

	fn sort_by<F>(&mut self, compare: F)
	where
		F: FnMut(&Self::Elem, &Self::Elem) -> Ordering,
	{
		self.borrow_mut().sort_by(compare)
	}

	fn len(&self) -> usize {
		self.borrow().len()
	}

	fn is_empty(&self) -> bool {
		self.borrow().is_empty()
	}
}

// === ArrayLike === //

pub trait ArrayLike: Sized + SliceLike + IntoIterator<Item = Self::Elem> {
	const DIM: usize;

	fn from_iter<I: IntoIterator<Item = Self::Elem>>(iter: I) -> Self;

	fn as_slice(&self) -> &[Self::Elem] {
		self.borrow()
	}

	fn as_slice_mut(&mut self) -> &mut [Self::Elem] {
		self.borrow_mut()
	}
}

impl<T, const N: usize> SliceLike for [T; N] {
	type Elem = T;
}

impl<T, const N: usize> ArrayLike for [T; N] {
	const DIM: usize = N;

	fn from_iter<I: IntoIterator<Item = Self::Elem>>(iter: I) -> Self {
		arr_from_iter(iter)
	}
}

// === VecLike === //

pub trait VecLike: SliceLike + IntoIterator<Item = Self::Elem> + Extend<Self::Elem> {
	fn push(&mut self, value: Self::Elem);

	fn pop(&mut self) -> Option<Self::Elem>;

	fn clear(&mut self);
}

impl<T> SliceLike for Vec<T> {
	type Elem = T;
}

impl<T> VecLike for Vec<T> {
	fn push(&mut self, value: Self::Elem) {
		self.push(value)
	}

	fn pop(&mut self) -> Option<Self::Elem> {
		self.pop()
	}

	fn clear(&mut self) {
		self.clear()
	}
}

impl<A: smallvec::Array> SliceLike for smallvec::SmallVec<A> {
	type Elem = A::Item;
}

impl<A: smallvec::Array> VecLike for smallvec::SmallVec<A> {
	fn push(&mut self, value: Self::Elem) {
		self.push(value)
	}

	fn pop(&mut self) -> Option<Self::Elem> {
		self.pop()
	}

	fn clear(&mut self) {
		self.clear()
	}
}
