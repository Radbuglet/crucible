use std::marker::PhantomData;

pub struct ConstSafeMutRef<'a, T: ?Sized> {
	_ty: PhantomData<&'a mut T>,
	ptr: *mut T,
}

// These are the same rules as for regular mutable references mutable.
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

	pub fn as_ref(&self) -> &mut T {
		unsafe { &mut *self.ptr }
	}

	pub fn to_ref(self) -> &'a mut T {
		unsafe { &mut *self.ptr }
	}
}
