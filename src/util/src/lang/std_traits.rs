use std::{
	borrow::{Borrow, BorrowMut},
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

// === ArrayLike === //

pub trait ArrayLike:
	Sized
	+ Borrow<[Self::Elem]>
	+ BorrowMut<[Self::Elem]>
	+ AsRef<[Self::Elem]>
	+ AsMut<[Self::Elem]>
	+ Index<usize, Output = Self::Elem>
	+ IndexMut<usize>
	+ IntoIterator<Item = Self::Elem>
{
	const DIM: usize;

	type Elem;

	fn from_iter<I: IntoIterator<Item = Self::Elem>>(iter: I) -> Self;

	fn as_slice(&self) -> &[Self::Elem] {
		self.borrow()
	}

	fn as_slice_mut(&mut self) -> &mut [Self::Elem] {
		self.borrow_mut()
	}
}

impl<T, const N: usize> ArrayLike for [T; N] {
	const DIM: usize = N;

	type Elem = T;

	fn from_iter<I: IntoIterator<Item = Self::Elem>>(iter: I) -> Self {
		arr_from_iter(iter)
	}
}
