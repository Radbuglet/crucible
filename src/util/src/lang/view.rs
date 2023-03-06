use std::ops::{Deref, DerefMut};

use crate::transparent;

transparent! {
	#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, PartialOrd, Ord)]
	pub struct View<T>(T)
	where {
		T: ?Sized
	};
}

impl<T: ?Sized> View<T> {
	pub fn from_ref(value: &T) -> &Self {
		Self::wrap_ref(value)
	}

	pub fn from_mut(value: &mut T) -> &mut Self {
		Self::wrap_mut(value)
	}
}

impl<T: ?Sized> Deref for View<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.raw
	}
}

impl<T: ?Sized> DerefMut for View<T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.raw
	}
}
