use std::{
	borrow::{Borrow, BorrowMut},
	ops::{Index, IndexMut}, cell::UnsafeCell,
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

// === UnsafeCellLike === //

pub unsafe trait UnsafeCellLike {
	type Inner: ?Sized;

	fn get(&self) -> *mut Self::Inner;

	fn into_inner(self) -> Self::Inner
	where
		Self::Inner: Sized;

	fn get_mut(&mut self) -> &mut Self::Inner {
		unsafe { &mut *self.get() }
	}

	unsafe fn get_ref_unchecked(&self) -> &Self::Inner {
		&*self.get()
	}

	#[allow(clippy::mut_from_ref)] // That's the users' problem.
	unsafe fn get_mut_unchecked(&self) -> &mut Self::Inner {
		&mut *self.get()
	}
}

unsafe impl<T: ?Sized> UnsafeCellLike for UnsafeCell<T> {
	type Inner = T;

	fn get(&self) -> *mut Self::Inner {
		// This is shadowed by the inherent `impl`.
		self.get()
	}

	fn into_inner(self) -> Self::Inner
	where
		Self::Inner: Sized,
	{
		// This is shadowed by the inherent `impl`.
		self.into_inner()
	}
}
