use std::marker::PhantomData;

pub struct ConstSafeMutPtr<'a, T: ?Sized> {
	_ty: PhantomData<&'a mut T>,
	ptr: *mut T,
}

// These are the same rules as for regular mutable references mutable.
unsafe impl<T: ?Sized + Send> Send for ConstSafeMutPtr<'_, T> {}
unsafe impl<T: ?Sized + Sync> Sync for ConstSafeMutPtr<'_, T> {}

impl<'a, T: ?Sized> From<&'a mut T> for ConstSafeMutPtr<'a, T> {
	fn from(ptr: &'a mut T) -> Self {
		Self {
			_ty: PhantomData,
			ptr,
		}
	}
}

impl<'a, T: ?Sized> ConstSafeMutPtr<'a, T> {
	pub fn new(ptr: &'a mut T) -> Self {
		ptr.into()
	}

	pub fn as_ref(self) -> &'a mut T {
		unsafe { &mut *self.ptr }
	}
}
