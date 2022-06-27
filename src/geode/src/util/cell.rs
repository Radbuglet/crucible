use std::{any::type_name, cell::UnsafeCell, fmt, hash, mem::MaybeUninit};

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

// === AssumeInit === //

/// An object that's usually initialized... except when it isn't.
///
/// Specifically, the value may be temporarily uninitialized during some internal invariant-breaking
/// procedures but the slot is assumed to be initialized during the rest of the time and thus,
///
/// - The destructor assumes that the inner value is initialized.
/// - Users can safely access the contents, even if the value is actually uninitialized.
///
/// In other words, we're emulating intuitive C-style "this value may be uninitialized but we'll
/// still assume that it is initialized if you try and access it" behavior.
///
/// Good luck, code review.
#[repr(transparent)]
pub struct AssumeInit<T>(MaybeUninit<T>);

impl<T> AssumeInit<T> {
	pub const fn new(value: T) -> Self {
		Self(MaybeUninit::new(value))
	}

	pub const unsafe fn uninit() -> Self {
		Self(MaybeUninit::uninit())
	}

	pub fn as_ptr(&self) -> *const T {
		self.0.as_ptr()
	}

	pub fn as_mut_ptr(&mut self) -> *mut T {
		self.0.as_mut_ptr()
	}

	pub fn as_ref(&self) -> &T {
		unsafe { self.0.assume_init_ref() }
	}

	pub fn as_mut(&mut self) -> &mut T {
		unsafe { self.0.assume_init_mut() }
	}

	pub unsafe fn read(&self) -> T {
		self.0.assume_init_read()
	}

	pub fn write(&mut self, value: T) {
		self.0.write(value);
	}

	pub fn unwrap(self) -> T {
		unsafe { self.0.assume_init_read() }
	}
}

impl<T: fmt::Debug> fmt::Debug for AssumeInit<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		self.as_ref().fmt(f)
	}
}

impl<T: Clone> Clone for AssumeInit<T> {
	fn clone(&self) -> Self {
		Self::new(self.as_ref().clone())
	}
}

impl<T: hash::Hash> hash::Hash for AssumeInit<T> {
	fn hash<H: hash::Hasher>(&self, state: &mut H) {
		self.as_ref().hash(state);
	}
}

impl<T: Eq> Eq for AssumeInit<T> {}

impl<T: PartialEq> PartialEq for AssumeInit<T> {
	fn eq(&self, other: &Self) -> bool {
		self.as_ref().eq(other.as_ref())
	}
}

impl<T> Drop for AssumeInit<T> {
	fn drop(&mut self) {
		unsafe { self.0.assume_init_drop() }
	}
}
