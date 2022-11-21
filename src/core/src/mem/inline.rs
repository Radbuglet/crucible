use std::{
	alloc::{self, Layout},
	mem::{self, ManuallyDrop},
};

use crate::mem::ptr::PointeeCastExt;

use super::ptr::leak_on_heap;

// === InlineStore === //

pub union InlineStore<C> {
	zst: (),
	_placeholder: ManuallyDrop<C>,
}

impl<C> InlineStore<C> {
	pub const fn can_hold_layout(layout: Layout) -> bool {
		// Alignment
		mem::align_of::<C>() >= layout.align()
			// Size
			&& mem::size_of::<C>() >= layout.size()
	}

	pub const fn can_hold<T>() -> bool {
		Self::can_hold_layout(Layout::new::<T>())
	}

	pub fn try_new<T>(value: T) -> Result<Self, T> {
		if Self::can_hold::<T>() {
			let mut target = Self { zst: () };

			unsafe {
				(&mut target as *mut Self).cast::<T>().write(value);
			}

			Ok(target)
		} else {
			Err(value)
		}
	}

	pub fn new<T>(value: T) -> Self {
		Self::try_new(value).ok().unwrap()
	}

	pub fn as_ptr<T>(&self) -> *const T {
		assert!(Self::can_hold::<T>());

		(self as *const Self).cast::<T>()
	}

	pub fn as_ptr_mut<T>(&mut self) -> *mut T {
		assert!(Self::can_hold::<T>());

		(self as *mut Self).cast::<T>()
	}

	pub unsafe fn get<T>(&self) -> &T {
		assert!(Self::can_hold::<T>());

		// Safety: provided by caller
		self.cast_ref_via_ptr(|ptr| ptr as *const T)
	}

	pub unsafe fn get_mut<T>(&mut self) -> &mut T {
		assert!(Self::can_hold::<T>());

		// Safety: provided by caller
		self.cast_mut_via_ptr(|ptr| ptr as *mut T)
	}

	pub unsafe fn into_inner<T>(self) -> T {
		unsafe { self.as_ptr::<T>().read() }
	}

	pub unsafe fn drop<T>(mut self) {
		self.drop_in_place::<T>();
	}

	pub unsafe fn drop_in_place<T>(&mut self) {
		let ptr = self.get_mut::<T>() as *mut T;

		ptr.drop_in_place();
	}
}

// === MaybeBoxed === //

pub union MaybeBoxed<C> {
	boxed: *mut u8,
	inlined: ManuallyDrop<InlineStore<C>>,
}

impl<C> MaybeBoxed<C> {
	pub fn new<T>(value: T) -> Self {
		match InlineStore::<C>::try_new(value) {
			Ok(inlined) => Self {
				inlined: ManuallyDrop::new(inlined),
			},
			Err(value) => {
				let boxed = leak_on_heap(value);
				Self {
					boxed: (boxed as *mut T).cast::<u8>(),
				}
			}
		}
	}

	pub unsafe fn get<T>(&self) -> &T {
		if InlineStore::<C>::can_hold::<T>() {
			self.inlined.get()
		} else {
			&*self.boxed.cast::<T>()
		}
	}

	pub unsafe fn get_mut<T>(&mut self) -> &mut T {
		if InlineStore::<C>::can_hold::<T>() {
			self.inlined.get_mut()
		} else {
			&mut *self.boxed.cast::<T>()
		}
	}

	pub unsafe fn copy(&self, layout: Layout) -> Self {
		if InlineStore::<C>::can_hold_layout(layout) {
			(self as *const Self).read()
		} else {
			if layout.size() == 0 {
				Self { boxed: self.boxed }
			} else {
				let new_boxed = alloc::alloc(layout);
				new_boxed.copy_from(self.boxed.cast::<u8>(), layout.size());
				Self { boxed: new_boxed }
			}
		}
	}

	pub unsafe fn deallocate_in_place(&mut self, layout: Layout) {
		if !InlineStore::<C>::can_hold_layout(layout) && layout.size() > 0 {
			alloc::dealloc(self.boxed, layout);
		}
	}

	pub unsafe fn into_inner<T>(self) -> T {
		if InlineStore::<C>::can_hold::<T>() {
			ManuallyDrop::into_inner(self.inlined).into_inner::<T>()
		} else {
			*Box::from_raw(self.boxed.cast::<T>())
		}
	}

	pub unsafe fn drop<T>(mut self) {
		self.drop_in_place::<T>();
	}

	pub unsafe fn drop_in_place<T>(&mut self) {
		if InlineStore::<C>::can_hold::<T>() {
			std::ptr::drop_in_place(self.inlined.get_mut::<T>());
		} else {
			drop(Box::from_raw(self.boxed));
		}
	}
}

// === AnyPtr === //

pub struct MaybeBoxedCopy<C> {
	layout: Layout,
	value: MaybeBoxed<C>,
}

impl<C> MaybeBoxedCopy<C> {
	pub fn new<T: Copy>(value: T) -> Self {
		Self {
			layout: Layout::new::<T>(),
			value: MaybeBoxed::new(value),
		}
	}

	pub unsafe fn get<T: Copy>(&self) -> T {
		*self.value.get::<T>()
	}
}

impl<C> Clone for MaybeBoxedCopy<C> {
	fn clone(&self) -> Self {
		Self {
			layout: self.layout,
			value: unsafe { self.value.copy(self.layout) },
		}
	}
}

impl<C> Drop for MaybeBoxedCopy<C> {
	fn drop(&mut self) {
		unsafe {
			self.value.deallocate_in_place(self.layout);
		}
	}
}
