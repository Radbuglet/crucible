use std::{
	fmt, hash,
	ops::{Deref, DerefMut},
};

use derive_where::derive_where;

const ACCESS_ERR_MSG: &str =
	"accessed value which was previously explicitly dropped or not yet bound.";

#[derive_where(Default)]
pub struct ExplicitlyBind<T>(Option<T>);

impl<T> ExplicitlyBind<T> {
	pub const fn new(value: T) -> Self {
		Self(Some(value))
	}

	pub const fn new_late() -> Self {
		Self(None)
	}

	pub fn bind(me: &mut Self, value: T) {
		assert!(
			me.0.is_none(),
			"Late-bound to a value that was already bound."
		);

		me.0 = Some(value);
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

impl<T: fmt::Debug> fmt::Debug for ExplicitlyBind<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		// Disambiguated by generic constraints
		(&**self).fmt(f)
	}
}

impl<T: Copy> Copy for ExplicitlyBind<T> {}

impl<T: Clone> Clone for ExplicitlyBind<T> {
	fn clone(&self) -> Self {
		(&**self).clone().into()
	}
}

impl<T: hash::Hash> hash::Hash for ExplicitlyBind<T> {
	fn hash<H: hash::Hasher>(&self, state: &mut H) {
		(&**self).hash(state);
	}
}

impl<T: Eq> Eq for ExplicitlyBind<T> {}

impl<T: PartialEq> PartialEq for ExplicitlyBind<T> {
	fn eq(&self, other: &Self) -> bool {
		&**self == &**other
	}
}

impl<T: Ord> Ord for ExplicitlyBind<T> {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		(&**self).cmp(&**other)
	}
}

impl<T: PartialOrd> PartialOrd for ExplicitlyBind<T> {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		(&**self).partial_cmp(&**other)
	}
}

impl<T> From<T> for ExplicitlyBind<T> {
	fn from(value: T) -> Self {
		Self(Some(value))
	}
}

impl<T> Deref for ExplicitlyBind<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		self.0.as_ref().expect(ACCESS_ERR_MSG)
	}
}

impl<T> DerefMut for ExplicitlyBind<T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		self.0.as_mut().expect(ACCESS_ERR_MSG)
	}
}
