use sealed::sealed;
use std::{any::type_name, cell::UnsafeCell, fmt, sync::LockResult};

// === Lock unwrapping === /

#[sealed]
pub trait LockGuardExt {
	type Inner;

	fn unpoison(self) -> Self::Inner;
}

#[sealed]
impl<G> LockGuardExt for LockResult<G> {
	type Inner = G;

	fn unpoison(self) -> Self::Inner {
		match self {
			Ok(value) => value,
			Err(err) => err.into_inner(),
		}
	}
}

// === Core cells === //

pub unsafe trait CellLike {
	type Value: ?Sized;

	fn new(value: Self::Value) -> Self
	where
		Self::Value: Sized;

	fn into_inner(self) -> Self::Value
	where
		Self::Value: Sized;

	fn get_mut(&mut self) -> &mut Self::Value;

	fn get(&self) -> *mut Self::Value;

	unsafe fn get_ref_unchecked(&self) -> &Self::Value {
		// Safety: provided by caller and the contract for `.get()`
		&*self.get()
	}

	unsafe fn get_mut_unchecked(&self) -> &mut Self::Value {
		// Safety: provided by caller and the contract for `.get()`
		&mut *self.get()
	}
}

pub trait Wrapper {
	type Wrapped;

	fn from_wrapped(value: Self::Wrapped) -> Self;
	fn into_wrapped(self) -> Self::Wrapped;
	fn get_wrapped(&self) -> &Self::Wrapped;
	fn get_wrapped_mut(&mut self) -> &mut Self::Wrapped;
}

unsafe impl<W: Wrapper<Wrapped = UnsafeCell<T>>, T: ?Sized> CellLike for W {
	type Value = T;

	fn new(value: Self::Value) -> Self
	where
		Self::Value: Sized,
	{
		Self::from_wrapped(UnsafeCell::new(value))
	}

	fn into_inner(self) -> Self::Value
	where
		Self::Value: Sized,
	{
		self.into_wrapped().into_inner()
	}

	fn get_mut(&mut self) -> &mut Self::Value {
		self.get_wrapped_mut().get_mut()
	}

	fn get(&self) -> *mut Self::Value {
		self.get_wrapped().get()
	}
}

/// A type of [UnsafeCell] that asserts that the given cell will only be accessed by one thread at a
/// given time.
#[derive(Default)]
pub struct MutexedUnsafeCell<T: ?Sized>(UnsafeCell<T>);

// Safety: Users can't do anything unsafe with this value without using `unsafe`. They take full
// responsibility for any extra danger when using this cell by asserting that they won't share a
// non-Sync value on several threads simultaneously.
unsafe impl<T: ?Sized> Sync for MutexedUnsafeCell<T> {}

impl<T> MutexedUnsafeCell<T> {
	// Shadows the non-const trait version.
	pub const fn new(value: T) -> Self {
		Self(UnsafeCell::new(value))
	}
}

impl<T> Wrapper for MutexedUnsafeCell<T> {
	type Wrapped = UnsafeCell<T>;

	fn from_wrapped(value: Self::Wrapped) -> Self {
		Self(value)
	}

	fn into_wrapped(self) -> Self::Wrapped {
		self.0
	}

	fn get_wrapped(&self) -> &Self::Wrapped {
		&self.0
	}

	fn get_wrapped_mut(&mut self) -> &mut Self::Wrapped {
		&mut self.0
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
