use std::any::type_name;
use std::cell::UnsafeCell;
use std::fmt::{Debug, Formatter};
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut};

// === UsuallySafeCell === //

pub struct UsuallySafeCell<T: ?Sized>(UnsafeCell<T>);

impl<T> UsuallySafeCell<T> {
	pub fn new(value: T) -> Self {
		Self(UnsafeCell::new(value))
	}

	pub fn into_inner(self) -> T {
		self.0.into_inner()
	}
}

impl<T: ?Sized> UsuallySafeCell<T> {
	#[allow(clippy::mut_from_ref)]
	pub unsafe fn unchecked_get_mut(&self) -> &mut T {
		&mut *self.0.get()
	}
}

impl<T: ?Sized> Deref for UsuallySafeCell<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		unsafe { &*self.0.get() }
	}
}

impl<T: ?Sized> DerefMut for UsuallySafeCell<T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		self.0.get_mut()
	}
}

impl<T: Debug> Debug for UsuallySafeCell<T> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		Debug::fmt(&**self, f)
	}
}

impl<T: Hash> Hash for UsuallySafeCell<T> {
	fn hash<H: Hasher>(&self, state: &mut H) {
		Hash::hash(&**self, state)
	}
}

impl<T: Eq> Eq for UsuallySafeCell<T> {}

impl<T: PartialEq> PartialEq for UsuallySafeCell<T> {
	fn eq(&self, other: &Self) -> bool {
		PartialEq::eq(&**self, other)
	}
}

impl<T: Default> Default for UsuallySafeCell<T> {
	fn default() -> Self {
		Self::new(Default::default())
	}
}

unsafe impl<T: Send> Send for UsuallySafeCell<T> {}
unsafe impl<T: Sync> Sync for UsuallySafeCell<T> {}

// === MakeSync === //

#[derive(Default)]
pub struct MakeSync<T: ?Sized>(T);

impl<T: ?Sized> Debug for MakeSync<T> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		f.debug_struct(format!("MakeSync<{}>", type_name::<T>()).as_str())
			.finish_non_exhaustive()
	}
}

impl<T: ?Sized> MakeSync<T> {
	pub fn get(&mut self) -> &mut T {
		&mut self.0
	}
}

unsafe impl<T: ?Sized + Send> Send for MakeSync<T> {}
unsafe impl<T: ?Sized + Send> Sync for MakeSync<T> {}
