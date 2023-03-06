use std::{
	mem::transmute,
	ops::{Deref, DerefMut},
};

// TODO: Find a way to use `transparent!` here.
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct View<T: ?Sized>(T);

impl<T: ?Sized> View<T> {
	pub fn from_ref(value: &T) -> &Self {
		unsafe {
			// Safety: we are `repr(transparent)` w.r.t `T`.
			transmute(value)
		}
	}

	pub fn from_mut(value: &mut T) -> &mut Self {
		unsafe {
			// Safety: we are `repr(transparent)` w.r.t `T`.
			transmute(value)
		}
	}
}

impl<T: ?Sized> Deref for View<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl<T: ?Sized> DerefMut for View<T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}
