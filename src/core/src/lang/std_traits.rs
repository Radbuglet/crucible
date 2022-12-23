use std::{
	borrow::{Borrow, BorrowMut},
	cell::{RefCell, UnsafeCell},
	ops::{Index, IndexMut},
};

use crate::mem::{array::arr_from_iter, c_enum::c_enum, ptr::PointeeCastExt};

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

// === CellLike === //

pub unsafe trait CellLike {
	type Inner: ?Sized;

	fn get_ptr(&self) -> *mut Self::Inner;

	fn into_inner(self) -> Self::Inner
	where
		Self::Inner: Sized;

	fn get_mut(&mut self) -> &mut Self::Inner {
		unsafe { &mut *self.get_ptr() }
	}

	unsafe fn get_ref_unchecked(&self) -> &Self::Inner {
		&*self.get_ptr()
	}

	#[allow(clippy::mut_from_ref)] // That's the users' problem.
	unsafe fn get_mut_unchecked(&self) -> &mut Self::Inner {
		&mut *self.get_ptr()
	}
}

unsafe impl<T: ?Sized> CellLike for UnsafeCell<T> {
	type Inner = T;

	fn get_ptr(&self) -> *mut Self::Inner {
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

unsafe impl<T: ?Sized> CellLike for RefCell<T> {
	type Inner = T;

	fn get_ptr(&self) -> *mut Self::Inner {
		// This is shadowed by the inherent `impl`.
		self.as_ptr()
	}

	fn into_inner(self) -> Self::Inner
	where
		Self::Inner: Sized,
	{
		// This is shadowed by the inherent `impl`.
		self.into_inner()
	}
}

pub unsafe trait TransparentCellLike: CellLike {
	fn from_mut(inner: &mut Self::Inner) -> &mut Self;
}

unsafe impl<T: ?Sized> TransparentCellLike for UnsafeCell<T> {
	fn from_mut(inner: &mut Self::Inner) -> &mut Self {
		unsafe { inner.cast_mut_via_ptr(|p| p as *mut Self) }
	}
}

// === RefCell Stuff === //

c_enum! {
	pub enum Mutability {
		Immutable,
		Mutable,
	}
}

impl Mutability {
	pub fn can_access_as(self, privileges: Self) -> bool {
		// Higher index => more privileges
		// i.e. if we can offer more than `privileges` is requesting, the check passes.
		self as usize >= privileges as usize
	}

	pub fn max_privileges(self, other: Self) -> Self {
		if self as usize > other as usize {
			self
		} else {
			other
		}
	}

	pub fn adverb(self) -> &'static str {
		match self {
			Self::Immutable => "immutably",
			Self::Mutable => "mutably",
		}
	}

	pub fn inverse(self) -> Self {
		match self {
			Self::Immutable => Self::Mutable,
			Self::Mutable => Self::Immutable,
		}
	}
}
