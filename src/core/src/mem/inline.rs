use std::mem::{self, ManuallyDrop};

use crate::mem::ptr::PointeeCastExt;

use super::ptr::leak_on_heap;

// === Inline Store === //

pub union InlineStore<C> {
	zst: (),
	_placeholder: ManuallyDrop<C>,
}

impl<C> InlineStore<C> {
	pub fn can_accommodate<T>() -> bool {
		// Alignment
		mem::align_of::<C>() >= mem::align_of::<T>()
			// Size
			&& mem::size_of::<C>() >= mem::size_of::<T>()
	}

	pub fn try_new_inline<T>(value: T) -> Result<Self, T> {
		if Self::can_accommodate::<T>() {
			let mut target = Self { zst: () };

			unsafe {
				(&mut target as *mut Self).cast::<T>().write(value);
			}

			Ok(target)
		} else {
			Err(value)
		}
	}

	pub fn new_inline<T>(value: T) -> Self {
		Self::try_new_inline(value).ok().unwrap()
	}

	pub unsafe fn decode_inline<T>(&self) -> &T {
		assert!(Self::can_accommodate::<T>());

		// Safety: provided by caller
		self.cast_ref_via_ptr(|ptr| ptr as *const T)
	}

	pub unsafe fn decode_inline_mut<T>(&mut self) -> &mut T {
		assert!(Self::can_accommodate::<T>());

		// Safety: provided by caller
		self.cast_mut_via_ptr(|ptr| ptr as *mut T)
	}

	pub unsafe fn drop_inline<T>(&mut self) {
		let ptr = self.decode_inline_mut::<T>() as *mut T;

		ptr.drop_in_place();
	}
}

// === Boxed Store === //

pub type BoxableInlineStore<C> = InlineStore<MaybeBoxed<C>>;

pub union MaybeBoxed<C> {
	_ptr: *mut C,
	_value: ManuallyDrop<C>,
}

unsafe impl<C: Send + Sync> Send for MaybeBoxed<C> {}
unsafe impl<C: Send + Sync> Sync for MaybeBoxed<C> {}

impl<C> BoxableInlineStore<C> {
	pub fn new_maybe_boxed<T>(value: T) -> Self {
		if Self::can_accommodate::<T>() {
			Self::new_inline(value)
		} else {
			Self::new_inline(leak_on_heap(value) as *mut T as *mut ())
		}
	}

	pub unsafe fn decode_maybe_boxed<T>(&self) -> &T {
		if Self::can_accommodate::<T>() {
			self.decode_inline::<T>()
		} else {
			let ptr = *self.decode_inline::<*mut T>();
			&*ptr
		}
	}

	pub unsafe fn decode_maybe_boxed_mut<T>(&mut self) -> &mut T {
		if Self::can_accommodate::<T>() {
			self.decode_inline_mut::<T>()
		} else {
			let ptr = *self.decode_inline::<*mut T>();
			&mut *ptr
		}
	}

	pub unsafe fn drop_maybe_boxed<T>(&mut self) {
		if Self::can_accommodate::<T>() {
			self.drop_inline::<T>()
		} else {
			let ptr = *self.decode_inline::<*mut T>();
			drop(Box::from_raw(ptr));
		}
	}
}
