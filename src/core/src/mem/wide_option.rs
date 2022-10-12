use crate::mem::transmute::{cast_mut_via_ptr, cast_ref_via_ptr};

use std::slice;

#[repr(transparent)]
pub struct WideOption<T>([T]);

impl<T> WideOption<T> {
	fn from_slice_mut(val: &mut [T]) -> &mut Self {
		unsafe {
			cast_mut_via_ptr(val, |ptr| {
				ptr as *mut WideOption<T> // repr(transparent)
			})
		}
	}

	pub fn some(val: &T) -> &Self {
		unsafe {
			cast_ref_via_ptr(slice::from_ref(val), |ptr| {
				ptr as *const WideOption<T> // repr(transparent)
			})
		}
	}

	pub fn some_mut(val: &mut T) -> &mut Self {
		Self::from_slice_mut(slice::from_mut(val))
	}

	pub fn none<'a>() -> &'a mut Self {
		Self::from_slice_mut(&mut [])
	}

	pub fn from_option(opt: Option<&T>) -> &Self {
		match opt {
			Some(val) => Self::some(val),
			None => Self::none(),
		}
	}

	pub fn from_option_mut(opt: Option<&mut T>) -> &mut Self {
		match opt {
			Some(val) => Self::some_mut(val),
			None => Self::none(),
		}
	}

	pub fn as_option(&self) -> Option<&T> {
		if !self.0.is_empty() {
			Some(&self.0[0])
		} else {
			None
		}
	}

	pub fn as_option_mut(&mut self) -> Option<&mut T> {
		if !self.0.is_empty() {
			Some(&mut self.0[0])
		} else {
			None
		}
	}

	pub fn is_some(&self) -> bool {
		self.as_option().is_some()
	}

	pub fn is_none(&self) -> bool {
		self.as_option().is_none()
	}

	pub fn unwrap_ref(&self) -> &T {
		self.as_option().unwrap()
	}

	pub fn unwrap_mut(&mut self) -> &mut T {
		self.as_option_mut().unwrap()
	}
}
