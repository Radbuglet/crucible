use core::any::type_name;
use core::fmt;

use bytemuck::TransparentWrapper;

// === AssertSync === //

#[derive(TransparentWrapper, Default)]
#[repr(transparent)]
pub struct AssertSync<T: ?Sized>(T);

// Safety: Users can't get an immutable reference to this value without using `unsafe`. They take full
// responsibility for any extra danger when using this cell by asserting that they won't share a
// non-Sync value on several threads simultaneously.
unsafe impl<T: ?Sized> Sync for AssertSync<T> {}

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
		&self.0
	}
}

// === OnlyMut === //

#[derive(Default)]
pub struct OnlyMut<T: ?Sized>(T);

impl<T> OnlyMut<T> {
	pub fn new(value: T) -> Self {
		Self(value)
	}
}

impl<T: ?Sized> fmt::Debug for OnlyMut<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct(format!("MakeSync<{}>", type_name::<T>()).as_str())
			.finish_non_exhaustive()
	}
}

impl<T: ?Sized> OnlyMut<T> {
	pub fn get(&mut self) -> &mut T {
		&mut self.0
	}
}

// Safe because we only give out references to the contents when a thread has exclusive access to the
// `OnlyMut` wrapper, thereby proving that the contents are not accessed by any other thread for the
// duration of the outer borrow.
unsafe impl<T: ?Sized + Send> Sync for OnlyMut<T> {}

// impl<T, U> CoerceUnsized<OnlyMut<U>> for OnlyMut<T> where T: CoerceUnsized<U> {}
