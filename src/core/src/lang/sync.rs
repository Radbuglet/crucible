use std::{
	cell::{RefCell, UnsafeCell},
	cmp, fmt, hash,
	ops::{Deref, DerefMut},
};

use bytemuck::TransparentWrapper;

use crate::mem::ptr::PointeeCastExt;

use super::std_traits::{CellLike, TransparentCellLike};

// === ExtRefCell === //

#[repr(transparent)]
pub struct ExtRefCell<T: ?Sized> {
	cell: RefCell<T>,
}

impl<T> ExtRefCell<T> {
	pub const fn new(value: T) -> Self {
		Self {
			cell: RefCell::new(value),
		}
	}

	pub fn into_inner(self) -> T {
		self.cell.into_inner()
	}
}

impl<T> From<RefCell<T>> for ExtRefCell<T> {
	fn from(value: RefCell<T>) -> Self {
		Self { cell: value }
	}
}

impl<T: Default> Default for ExtRefCell<T> {
	fn default() -> Self {
		Self {
			cell: Default::default(),
		}
	}
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for ExtRefCell<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		(&**self).fmt(f)
	}
}

impl<T: Clone> Clone for ExtRefCell<T> {
	fn clone(&self) -> Self {
		Self {
			cell: RefCell::new((&**self).clone()),
		}
	}
}

impl<T: ?Sized + Eq> Eq for ExtRefCell<T> {}

impl<T: ?Sized + PartialEq> PartialEq for ExtRefCell<T> {
	fn eq(&self, other: &Self) -> bool {
		&*self == &*other
	}
}

impl<T: ?Sized + PartialOrd> PartialOrd for ExtRefCell<T> {
	fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
		(&**self).partial_cmp(&**other)
	}
}

impl<T: ?Sized + Ord> Ord for ExtRefCell<T> {
	fn cmp(&self, other: &Self) -> cmp::Ordering {
		(&**self).cmp(&**other)
	}
}

impl<T: ?Sized + hash::Hash> hash::Hash for ExtRefCell<T> {
	fn hash<H: hash::Hasher>(&self, state: &mut H) {
		(&**self).hash(state);
	}
}

// When using this wrapper, we act as if we were employing pure exterior mutability.
unsafe impl<T: Sync> Sync for ExtRefCell<T> {}

impl<T: ?Sized> Deref for ExtRefCell<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		unsafe {
			// Safety: `ExtRefCell` containers employ exterior mutability
			self.cell.get_ref_unchecked()
		}
	}
}

impl<T: ?Sized> DerefMut for ExtRefCell<T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		unsafe {
			// Safety: `ExtRefCell` containers employ exterior mutability
			self.cell.get_mut_unchecked()
		}
	}
}

// === SyncUnsafeCell === //

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

unsafe impl<T: ?Sized> CellLike for SyncUnsafeCell<T> {
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

unsafe impl<T: ?Sized> TransparentCellLike for SyncUnsafeCell<T> {
	fn from_mut(inner: &mut Self::Inner) -> &mut Self {
		unsafe { inner.cast_mut_via_ptr(|p| p as *mut Self) }
	}
}
