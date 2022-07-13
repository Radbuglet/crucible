#[derive(Debug, Clone, Hash, Eq, PartialEq, Ord, PartialOrd, Default)]
#[repr(transparent)]
pub struct Owned<T: Destructible>(T);

impl<T: Destructible> From<T> for Owned<T> {
	fn from(inner: T) -> Self {
		Self::new(inner)
	}
}

impl<T: Destructible> Owned<T> {
	pub fn new(inner: T) -> Self {
		Self(inner)
	}

	pub fn map_owned<F, R: Destructible>(self, f: F) -> Owned<R>
	where
		F: FnOnce(T) -> R,
	{
		Owned::new(f(self.manually_destruct()))
	}

	pub fn manually_destruct(self) -> T {
		let inner = self.0;
		std::mem::forget(self);
		inner
	}

	pub fn weak_copy(&self) -> T {
		self.0
	}

	pub fn to_guard_ref_pair(self) -> (Self, T) {
		let copy = self.weak_copy();
		(self, copy)
	}
}

impl<T: Destructible> Drop for Owned<T> {
	fn drop(&mut self) {
		self.0.destruct();
	}
}

pub trait Destructible: Copy {
	fn destruct(self);
}
