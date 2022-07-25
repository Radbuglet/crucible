use bytemuck::TransparentWrapper;

// === Owned === //

pub trait Destructible: Copy {
	fn destruct(self);
}

#[derive(Debug, Clone, Hash, Eq, PartialEq, Ord, PartialOrd, Default, TransparentWrapper)]
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

	pub fn try_map_owned<F, R, E>(self, f: F) -> Result<Owned<R>, E>
	where
		F: FnOnce(T) -> Result<R, E>,
		R: Destructible,
	{
		Ok(Owned::new(f(self.manually_destruct())?))
	}

	pub fn map_owned<F, R>(self, f: F) -> Owned<R>
	where
		F: FnOnce(T) -> R,
		R: Destructible,
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

	pub fn weak_copy_ref(&self) -> &T {
		&self.0
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

// === MaybeOwned === //

// TODO: Forward relevant `impl`'s in `MaybeOwned`.
#[derive(Debug, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub enum MaybeOwned<T: Destructible> {
	Owned(Owned<T>),
	Weak(T),
}

impl<T: Destructible> MaybeOwned<T> {
	pub fn try_map<F, R, E>(self, f: F) -> Result<MaybeOwned<R>, E>
	where
		F: FnOnce(T) -> Result<R, E>,
		R: Destructible,
	{
		match self {
			MaybeOwned::Owned(owned) => Ok(MaybeOwned::Owned(owned.try_map_owned(f)?)),
			MaybeOwned::Weak(weak) => Ok(MaybeOwned::Weak(f(weak)?)),
		}
	}

	pub fn map<F, R>(self, f: F) -> MaybeOwned<R>
	where
		F: FnOnce(T) -> R,
		R: Destructible,
	{
		match self {
			MaybeOwned::Owned(owned) => MaybeOwned::Owned(owned.map_owned(f)),
			MaybeOwned::Weak(weak) => MaybeOwned::Weak(f(weak)),
		}
	}

	pub fn weak_copy(&self) -> T {
		match self {
			MaybeOwned::Owned(owned) => owned.weak_copy(),
			MaybeOwned::Weak(weak) => *weak,
		}
	}

	pub fn weak_copy_ref(&self) -> &T {
		match self {
			MaybeOwned::Owned(owned) => owned.weak_copy_ref(),
			MaybeOwned::Weak(weak) => weak,
		}
	}

	pub fn manually_destruct(self) -> T {
		match self {
			MaybeOwned::Owned(owned) => owned.manually_destruct(),
			MaybeOwned::Weak(weak) => weak,
		}
	}

	pub fn is_owned(&self) -> bool {
		matches!(self, Self::Owned(_))
	}
}

impl<T: Destructible> From<T> for MaybeOwned<T> {
	fn from(weak: T) -> Self {
		Self::Weak(weak)
	}
}

impl<T: Destructible> From<Owned<T>> for MaybeOwned<T> {
	fn from(owned: Owned<T>) -> Self {
		Self::Owned(owned)
	}
}
