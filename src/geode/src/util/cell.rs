use std::any::type_name;
use std::cell::UnsafeCell;
use std::fmt::{Debug, Formatter};
use std::hash::{Hash, Hasher};
use std::ops::{CoerceUnsized, Deref, DerefMut};
use std::ptr::NonNull;

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

impl<T, U> CoerceUnsized<UsuallySafeCell<U>> for UsuallySafeCell<T> where T: CoerceUnsized<U> {}

// === OnlyMut === //

#[derive(Default)]
pub struct OnlyMut<T: ?Sized>(T);

impl<T> OnlyMut<T> {
	pub fn new(value: T) -> Self {
		Self(value)
	}
}

impl<T: ?Sized> Debug for OnlyMut<T> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		f.debug_struct(format!("MakeSync<{}>", type_name::<T>()).as_str())
			.finish_non_exhaustive()
	}
}

impl<T: ?Sized> OnlyMut<T> {
	pub fn get(&mut self) -> &mut T {
		&mut self.0
	}
}

// Safe because we only give out mutable references to the contents.
unsafe impl<T: ?Sized + Send> Sync for OnlyMut<T> {}

impl<T, U> CoerceUnsized<OnlyMut<U>> for OnlyMut<T> where T: CoerceUnsized<U> {}

// === SyncUnsafeCell === //

/// An [UnsafeCell] that asserts that its users are properly synchronizing access to its contents
/// across threads. This cell makes no assertion that the returned immutable references will be used
/// in a thread-safe manner so `T: Sync` in order for `SyncUnsafeCell<T>: Sync`. You can use
/// [SyncUnsafeCellMut]—which will always be `Sync`—if you know you only need a mutable reference
/// to the contents.
pub struct SyncUnsafeCell<T: ?Sized>(UnsafeCell<T>);

unsafe impl<T: ?Sized + Sync> Sync for SyncUnsafeCell<T> {}

impl<T: Default> Default for SyncUnsafeCell<T> {
	fn default() -> Self {
		Self::new(Default::default())
	}
}

impl<T> SyncUnsafeCell<T> {
	pub fn new(value: T) -> Self {
		Self(UnsafeCell::new(value))
	}

	pub fn into_inner(self) -> T {
		self.0.into_inner()
	}
}

impl<T: ?Sized> SyncUnsafeCell<T> {
	pub fn get(&self) -> *mut T {
		self.0.get()
	}

	pub fn get_mut(&mut self) -> &mut T {
		self.0.get_mut()
	}

	pub unsafe fn get_ref_unchecked(&self) -> &T {
		&*self.0.get()
	}

	#[allow(clippy::mut_from_ref)]
	pub unsafe fn get_mut_unchecked(&self) -> &mut T {
		&mut *self.0.get()
	}
}

impl<T, U> CoerceUnsized<SyncUnsafeCell<U>> for SyncUnsafeCell<T> where T: CoerceUnsized<U> {}

// === SyncUnsafeCellMut === //

/// An [UnsafeCell] that asserts that its users are properly synchronizing access to its contents
/// across threads. This cell makes no assertion that the returned immutable references will be used
/// in a thread-safe manner so `T: Sync` in order for `SyncUnsafeCell<T>: Sync`. You can use
/// [SyncUnsafeCellMut]—which will always be `Sync`—if you know you only need a mutable reference
/// to the contents.
pub struct SyncUnsafeCellMut<T: ?Sized>(UnsafeCell<T>);

unsafe impl<T: ?Sized> Sync for SyncUnsafeCellMut<T> {}

impl<T: Default> Default for SyncUnsafeCellMut<T> {
	fn default() -> Self {
		Self::new(Default::default())
	}
}

impl<T> SyncUnsafeCellMut<T> {
	pub fn new(value: T) -> Self {
		Self(UnsafeCell::new(value))
	}

	pub fn into_inner(self) -> T {
		self.0.into_inner()
	}
}

impl<T: ?Sized> SyncUnsafeCellMut<T> {
	pub fn get_mut(&mut self) -> &mut T {
		self.0.get_mut()
	}

	#[allow(clippy::mut_from_ref)]
	pub unsafe fn get_mut_unchecked(&self) -> &mut T {
		&mut *self.0.get()
	}
}

impl<T, U> CoerceUnsized<SyncUnsafeCellMut<U>> for SyncUnsafeCellMut<T> where T: CoerceUnsized<U> {}

// === SyncPtr === //

/// Asserts that a given pointer has taken synchronization into account.
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct SyncPtr<T: PtrType>(pub T);

unsafe impl<T: PtrType> Send for SyncPtr<T> {}
unsafe impl<T: PtrType> Sync for SyncPtr<T> {}

impl<T: PtrType> Deref for SyncPtr<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl<T: PtrType> DerefMut for SyncPtr<T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

pub unsafe trait PtrType: Sized {}

unsafe impl<T: ?Sized> PtrType for *const T {}
unsafe impl<T: ?Sized> PtrType for *mut T {}
unsafe impl<T: ?Sized> PtrType for NonNull<T> {}
