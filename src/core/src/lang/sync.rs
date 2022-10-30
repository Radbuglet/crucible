use core::{any, fmt};
use std::{cell::UnsafeCell, marker::PhantomData};

use bytemuck::TransparentWrapper;

use super::{marker::PhantomNoSendOrSync, std_traits::UnsafeCellLike};

#[derive(Default)]
pub struct AssertSync<T: ?Sized>(T);

// Safety: users can only unwrap references to `AssertSync` via the unsafe `AssertSync::get` method.
unsafe impl<T: ?Sized> Sync for AssertSync<T> {}

impl<T: ?Sized> fmt::Debug for AssertSync<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct(format!("AssertSync<{}>", any::type_name::<T>()).as_str())
			.finish_non_exhaustive()
	}
}

impl<T> AssertSync<T> {
	pub const fn new(value: T) -> Self {
		Self(value)
	}

	pub fn into_inner(self) -> T {
		self.0
	}
}

impl<T: ?Sized> AssertSync<T> {
	pub unsafe fn get(&self) -> &T {
		// Safety: provided by caller
		&self.0
	}
}

impl<T: ?Sized> AssertSync<T> {
	pub fn get_mut(&mut self) -> &mut T {
		&mut self.0
	}
}

// === MutexedUnsafeCell === //

/// A type of [UnsafeCell] that asserts that access to the given cell will be properly synchronized.
#[derive(Default, TransparentWrapper)]
#[repr(transparent)]
pub struct SyncUnsafeCell<T: ?Sized>(UnsafeCell<T>);

// Safety: Users can't get an immutable reference to this value without using `unsafe`. They take full
// responsibility for any extra danger when using this cell by asserting that they won't share a
// non-Sync value on several threads simultaneously. We require `Send` in this bound as an extra
// precaution because users could theoretically use the cell's newfound `Sync` superpowers to move a
// non-`Send` `T` instance to another thread via a mutable reference to it and people are only really
// promising that they'll federate access properly.
unsafe impl<T: ?Sized + Send> Sync for SyncUnsafeCell<T> {}

impl<T> SyncUnsafeCell<T> {
	pub const fn new(value: T) -> Self {
		Self(UnsafeCell::new(value))
	}

	pub fn into_inner(self) -> T {
		self.0.into_inner()
	}
}

unsafe impl<T: ?Sized> UnsafeCellLike for SyncUnsafeCell<T> {
	type Inner = T;

	fn get_ptr(&self) -> *mut Self::Inner {
		self.0.get()
	}

	fn into_inner(self) -> Self::Inner
	where
		Self::Inner: Sized,
	{
		self.0.into_inner()
	}
}

// === MutexedPtr === //

pub type SendPtrMut<T> = MutexedPtr<*mut T>;
pub type SendPtrRef<T> = MutexedPtr<*const T>;

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct MutexedPtr<P> {
	_ty: PhantomNoSendOrSync,
	ptr: P,
}

impl<P> From<P> for MutexedPtr<P> {
	fn from(ptr: P) -> Self {
		Self {
			_ty: PhantomData,
			ptr,
		}
	}
}

impl<P> MutexedPtr<P> {
	pub fn ptr(self) -> P {
		self.ptr
	}
}

unsafe impl<T: ?Sized + Send> Send for MutexedPtr<*mut T> {}
unsafe impl<T: ?Sized + Sync> Sync for MutexedPtr<*mut T> {}

unsafe impl<T: ?Sized + Send + Sync> Send for MutexedPtr<*const T> {}
unsafe impl<T: ?Sized + Sync> Sync for MutexedPtr<*const T> {}
