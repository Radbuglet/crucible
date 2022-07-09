use std::{
	mem::ManuallyDrop,
	ops::{Deref, DerefMut},
};

#[derive(Debug, Clone, Hash, Eq, PartialEq, Ord, PartialOrd, Default)]
#[repr(transparent)]
pub struct Owned<T: Destructible>(ManuallyDrop<T>);

impl<T: Destructible> From<T> for Owned<T> {
	fn from(inner: T) -> Self {
		Self::new(inner)
	}
}

impl<T: Destructible> Owned<T> {
	pub fn new(inner: T) -> Self {
		Self(ManuallyDrop::new(inner))
	}

	pub fn manually_destruct(mut self) -> T {
		let inner = unsafe { ManuallyDrop::take(&mut self.0) };
		std::mem::forget(self);
		inner
	}
}

impl<T: Destructible> Deref for Owned<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl<T: Destructible> DerefMut for Owned<T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl<T: Destructible> Drop for Owned<T> {
	fn drop(&mut self) {
		let value = unsafe { ManuallyDrop::take(&mut self.0) };
		value.destruct();
	}
}

pub trait Destructible {
	fn destruct(self);
}
