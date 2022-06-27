use std::{any::type_name, cell::UnsafeCell, fmt};

// === MutexedUnsafeCell === //

/// A type of [UnsafeCell] that asserts that the given cell will only be accessed by one thread at a
/// given time.
#[derive(Default)]
#[repr(transparent)]
pub struct MutexedUnsafeCell<T: ?Sized>(UnsafeCell<T>);

// Safety: Users can't get an immutable reference to this value without using `unsafe`. They take full
// responsibility for any extra danger when using this cell by asserting that they won't share a
// non-Sync value on several threads simultaneously.
unsafe impl<T: ?Sized> Sync for MutexedUnsafeCell<T> {}

impl<T> MutexedUnsafeCell<T> {
	pub const fn new(value: T) -> Self {
		Self(UnsafeCell::new(value))
	}

	pub fn into_inner(self) -> T {
		self.0.into_inner()
	}
}

impl<T: ?Sized> MutexedUnsafeCell<T> {
	pub fn get_mut(&mut self) -> &mut T {
		self.0.get_mut()
	}

	pub fn get(&self) -> *mut T {
		self.0.get()
	}

	pub unsafe fn get_ref_unchecked(&self) -> &T {
		&*self.get()
	}

	pub unsafe fn get_mut_unchecked(&self) -> &mut T {
		&mut *self.get()
	}
}

// impl<T, U> CoerceUnsized<MutexedUnsafeCell<U>> for MutexedUnsafeCell<T> where T: CoerceUnsized<U> {}

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
