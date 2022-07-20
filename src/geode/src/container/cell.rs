use std::cell::Cell;

use crate::core::owned::{Destructible, Owned};

pub unsafe trait CopyInner {
	type Inner;

	fn copy_inner(of: &Self) -> Self::Inner;
}

unsafe impl<T: Destructible> CopyInner for Owned<T> {
	type Inner = T;

	fn copy_inner(of: &Self) -> Self::Inner {
		of.weak_copy()
	}
}

unsafe impl<T: CopyInner> CopyInner for Option<T> {
	type Inner = Option<T::Inner>;

	fn copy_inner(of: &Self) -> Self::Inner {
		of.as_ref().map(CopyInner::copy_inner)
	}
}

mod cell_ext {
	pub trait Sealed {}
}

pub trait CellExt: cell_ext::Sealed {
	type Inner;

	fn get_inner(&self) -> Self::Inner;
}

impl<T: CopyInner> cell_ext::Sealed for Cell<T> {}

impl<T: CopyInner> CellExt for Cell<T> {
	type Inner = T::Inner;

	fn get_inner(&self) -> Self::Inner {
		let inner = unsafe { &*self.as_ptr() };
		T::copy_inner(inner)
	}
}
