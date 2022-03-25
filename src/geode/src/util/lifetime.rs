use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

// === Take === //

pub trait Take<T>: Sized {
	fn take_owned(self) -> T;
	fn take_ref(&self) -> &T;
}

impl<T> Take<T> for T {
	fn take_owned(self) -> T {
		self
	}

	fn take_ref(&self) -> &T {
		self
	}
}

impl<T: Clone> Take<T> for &'_ T {
	fn take_owned(self) -> T {
		Clone::clone(self)
	}

	fn take_ref(&self) -> &T {
		self
	}
}

// === Reference limiting === //

/// Limits the lifetime during which `T` is accessible to `'a`.
#[derive(Debug)]
pub struct LimitLifetimeMut<'a, T> {
	// The lifetime is covariant (e.g. 'static -> 'a)
	_ty: PhantomData<&'a T>,

	// The actual contents whose lifetime is being limited.
	value: T,
}

impl<T> LimitLifetimeMut<'_, T> {
	pub fn new(value: T) -> Self {
		Self {
			_ty: PhantomData,
			value,
		}
	}
}

impl<T> Deref for LimitLifetimeMut<'_, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.value
	}
}

impl<T> DerefMut for LimitLifetimeMut<'_, T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.value
	}
}

/// Limits the lifetime during which `T` is accessible to `'a` and only exposes an immutable
/// reference to the contents.
#[derive(Debug)]
pub struct LimitLifetimeRef<'a, T> {
	// The lifetime is covariant (e.g. 'static -> 'a)
	_ty: PhantomData<&'a T>,

	// The actual contents whose lifetime is being limited.
	value: T,
}

impl<'a, T> LimitLifetimeRef<'a, T> {
	pub fn new(value: T) -> Self {
		Self {
			_ty: PhantomData,
			value,
		}
	}
}

impl<T> Deref for LimitLifetimeRef<'_, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.value
	}
}

/// A wrapper around `&'a T` that's not [Copy] or [Clone].
/// This ensures that the reference can only be accessed for as long as it is accessible.
#[derive(Debug, Hash, Eq, PartialEq)]
pub struct NoCopyRef<'a, T: ?Sized>(&'a T);

impl<'a, T: ?Sized> NoCopyRef<'a, T> {
	pub fn new(val: &'a T) -> Self {
		Self(val)
	}

	pub fn clone(&'_ self) -> NoCopyRef<'_, T> {
		NoCopyRef(self.0)
	}
}

impl<T: ?Sized> Deref for NoCopyRef<'_, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		self.0
	}
}
