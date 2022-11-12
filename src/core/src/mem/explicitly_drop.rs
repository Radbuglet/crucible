use std::{
	fmt, hash,
	ops::{Deref, DerefMut},
};

const ACCESS_ERR_MSG: &str = "accessed value which was previously explicitly dropped";

pub struct ExplicitlyDrop<T>(Option<T>);

impl<T> ExplicitlyDrop<T> {
	pub const fn new(value: T) -> Self {
		Self(Some(value))
	}

	pub fn into_inner(me: Self) -> T {
		me.0.expect(ACCESS_ERR_MSG)
	}

	pub fn extract(me: &mut Self) -> T {
		me.0.take().expect(ACCESS_ERR_MSG)
	}

	pub fn is_alive(me: &Self) -> bool {
		me.0.is_some()
	}

	pub fn drop(me: &mut Self) {
		let _ = Self::extract(me);
	}
}

impl<T: fmt::Debug> fmt::Debug for ExplicitlyDrop<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		// Disambiguated by generic constraints
		(&**self).fmt(f)
	}
}

impl<T: Copy> Copy for ExplicitlyDrop<T> {}

impl<T: Clone> Clone for ExplicitlyDrop<T> {
	fn clone(&self) -> Self {
		(&**self).clone().into()
	}
}

impl<T: hash::Hash> hash::Hash for ExplicitlyDrop<T> {
	fn hash<H: hash::Hasher>(&self, state: &mut H) {
		(&**self).hash(state);
	}
}

impl<T: Eq> Eq for ExplicitlyDrop<T> {}

impl<T: PartialEq> PartialEq for ExplicitlyDrop<T> {
	fn eq(&self, other: &Self) -> bool {
		&**self == &**other
	}
}

impl<T: Ord> Ord for ExplicitlyDrop<T> {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		(&**self).cmp(&**other)
	}
}

impl<T: PartialOrd> PartialOrd for ExplicitlyDrop<T> {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		(&**self).partial_cmp(&**other)
	}
}

impl<T: Default> Default for ExplicitlyDrop<T> {
	fn default() -> Self {
		T::default().into()
	}
}

impl<T> From<T> for ExplicitlyDrop<T> {
	fn from(value: T) -> Self {
		Self(Some(value))
	}
}

impl<T> Deref for ExplicitlyDrop<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		self.0.as_ref().expect(ACCESS_ERR_MSG)
	}
}

impl<T> DerefMut for ExplicitlyDrop<T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		self.0.as_mut().expect(ACCESS_ERR_MSG)
	}
}