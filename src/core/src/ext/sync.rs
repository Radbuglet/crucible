use core::{any, fmt};
use std::marker::PhantomData;

use crate::marker::PhantomNoSendOrSync;

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

impl<T: ?Sized + Send> AssertSync<T> {
	pub fn get_mut(&mut self) -> &mut T {
		// Safety: `&mut T: Send` so long as `T: Send`.
		&mut self.0
	}
}

// impl<T, U> CoerceUnsized<OnlyMut<U>> for OnlyMut<T> where T: CoerceUnsized<U> {}

pub type PMutSend<T> = MutexedPtr<*mut T>;
pub type PRefSend<T> = MutexedPtr<*const T>;

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
