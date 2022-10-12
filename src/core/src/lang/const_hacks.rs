use std::{
	marker::PhantomData,
	ops::{Deref, DerefMut},
};

pub struct ConstSafeMutRef<'a, T: ?Sized> {
	_ty: PhantomData<&'a mut T>,
	ptr: *mut T,
}

// These are the same rules as for regular mutable references.
unsafe impl<T: ?Sized + Send> Send for ConstSafeMutRef<'_, T> {}
unsafe impl<T: ?Sized + Sync> Sync for ConstSafeMutRef<'_, T> {}

impl<'a, T: ?Sized> From<&'a mut T> for ConstSafeMutRef<'a, T> {
	fn from(ptr: &'a mut T) -> Self {
		Self {
			_ty: PhantomData,
			ptr,
		}
	}
}

impl<'a, T: ?Sized> ConstSafeMutRef<'a, T> {
	pub fn new(ptr: &'a mut T) -> Self {
		ptr.into()
	}

	pub fn into_ref(self) -> &'a mut T {
		unsafe { &mut *self.ptr }
	}
}

impl<'a, T> Deref for ConstSafeMutRef<'a, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		unsafe { &*self.ptr }
	}
}

impl<'a, T> DerefMut for ConstSafeMutRef<'a, T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		unsafe { &mut *self.ptr }
	}
}
