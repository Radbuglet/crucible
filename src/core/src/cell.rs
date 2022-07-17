use core::cell::{Ref, RefMut};

use bytemuck::TransparentWrapper;

use crate::lifetime::try_transform;

// === RefCell extensions === //

pub fn filter_map_ref<T, U, F>(orig: Ref<T>, f: F) -> Result<Ref<U>, Ref<T>>
where
	F: FnOnce(&T) -> Option<&U>,
{
	// Thanks to `kpreid` for the awesome insight behind this technique!
	let backup = Ref::clone(&orig);
	let mapped = Ref::map(orig, |orig| match f(orig) {
		Some(mapped) => std::slice::from_ref(mapped),
		None => &[],
	});

	if mapped.len() > 0 {
		Ok(Ref::map(mapped, |slice| &slice[0]))
	} else {
		Err(backup)
	}
}

pub fn filter_map_mut<T, U, F>(orig: RefMut<T>, f: F) -> Result<RefMut<U>, RefMut<T>>
where
	F: FnOnce(&mut T) -> Option<&mut U>,
{
	// Utils
	// Thanks, again, to `kpreid` for helping me make the original implementation of this safe.
	trait Either<U, T> {
		fn as_result(&mut self) -> Result<&mut U, &mut T>;
	}

	#[derive(TransparentWrapper)]
	#[repr(transparent)]
	struct Success<U>(U);

	impl<U, T> Either<U, T> for Success<U> {
		fn as_result(&mut self) -> Result<&mut U, &mut T> {
			Ok(&mut self.0)
		}
	}

	#[derive(TransparentWrapper)]
	#[repr(transparent)]
	struct Failure<T>(T);

	impl<U, T> Either<U, T> for Failure<T> {
		fn as_result(&mut self) -> Result<&mut U, &mut T> {
			Err(&mut self.0)
		}
	}

	// Actual implementation
	let mut mapped = RefMut::map(orig, |orig| match try_transform(orig, f) {
		Ok(mapped) => {
			<Success<U> as TransparentWrapper<U>>::wrap_mut(mapped) as &mut dyn Either<U, T>
		}
		Err(orig) => <Failure<T> as TransparentWrapper<T>>::wrap_mut(orig) as &mut dyn Either<U, T>,
	});

	match mapped.as_result().is_ok() {
		true => Ok(RefMut::map(mapped, |val| val.as_result().ok().unwrap())),
		false => Err(RefMut::map(mapped, |val| val.as_result().err().unwrap())),
	}
}

use std::{any::type_name, cell::UnsafeCell, fmt};

// === MutexedUnsafeCell === //

/// A type of [UnsafeCell] that asserts that the given cell will only be accessed by one thread at a
/// given time.
#[derive(Default)]
#[repr(transparent)]
pub struct MutexedUnsafeCell<T: ?Sized>(UnsafeCell<T>);

// Safety: Users can't get an immutable reference to this value without using `unsafe`. They take full
// responsibility for any extra danger when using this cell by asserting that they won't share a
// non-Sync value on several threads simultaneously.
unsafe impl<T: ?Sized> Sync for MutexedUnsafeCell<T> {}

impl<T> MutexedUnsafeCell<T> {
	pub const fn new(value: T) -> Self {
		Self(UnsafeCell::new(value))
	}

	pub fn into_inner(self) -> T {
		self.0.into_inner()
	}
}

impl<T: ?Sized> MutexedUnsafeCell<T> {
	pub fn get_mut(&mut self) -> &mut T {
		self.0.get_mut()
	}

	pub fn get(&self) -> *mut T {
		self.0.get()
	}

	pub unsafe fn get_ref_unchecked(&self) -> &T {
		&*self.get()
	}

	#[allow(clippy::mut_from_ref)] // That's the users' problem.
	pub unsafe fn get_mut_unchecked(&self) -> &mut T {
		&mut *self.get()
	}
}

// impl<T, U> CoerceUnsized<MutexedUnsafeCell<U>> for MutexedUnsafeCell<T> where T: CoerceUnsized<U> {}

// === OnlyMut === //

#[derive(Default)]
pub struct OnlyMut<T: ?Sized>(T);

impl<T> OnlyMut<T> {
	pub fn new(value: T) -> Self {
		Self(value)
	}
}

impl<T: ?Sized> fmt::Debug for OnlyMut<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct(format!("MakeSync<{}>", type_name::<T>()).as_str())
			.finish_non_exhaustive()
	}
}

impl<T: ?Sized> OnlyMut<T> {
	pub fn get(&mut self) -> &mut T {
		&mut self.0
	}
}

// Safe because we only give out references to the contents when a thread has exclusive access to the
// `OnlyMut` wrapper, thereby proving that the contents are not accessed by any other thread for the
// duration of the outer borrow.
unsafe impl<T: ?Sized + Send> Sync for OnlyMut<T> {}

// impl<T, U> CoerceUnsized<OnlyMut<U>> for OnlyMut<T> where T: CoerceUnsized<U> {}
