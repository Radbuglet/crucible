use core::cell::{Ref, RefMut, UnsafeCell};

use bytemuck::TransparentWrapper;

use crate::{ext::lifetime::try_transform, wide_option::WideOption};

// === RefCell extensions === //

pub fn filter_map_ref<T, U, F>(orig: Ref<T>, f: F) -> Result<Ref<U>, Ref<T>>
where
	F: FnOnce(&T) -> Option<&U>,
{
	// Thanks to `kpreid` for the awesome insight behind this technique!
	let backup = Ref::clone(&orig);
	let mapped = Ref::map(orig, |orig| match f(orig) {
		Some(mapped) => WideOption::some(mapped),
		None => WideOption::none(),
	});

	if mapped.is_some() {
		Ok(Ref::map(mapped, |mapped| mapped.unwrap_ref()))
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
		Ok(mapped) => Success::wrap_mut(mapped) as &mut dyn Either<U, T>,
		Err(orig) => Failure::wrap_mut(orig) as &mut dyn Either<U, T>,
	});

	match mapped.as_result().is_ok() {
		true => Ok(RefMut::map(mapped, |val| val.as_result().ok().unwrap())),
		false => Err(RefMut::map(mapped, |val| val.as_result().err().unwrap())),
	}
}

// === UnsafeCellExt === //

pub unsafe trait UnsafeCellExt {
	type Inner: ?Sized;

	fn get(&self) -> *mut Self::Inner;

	fn get_mut(&mut self) -> &mut Self::Inner {
		unsafe { &mut *self.get() }
	}

	unsafe fn get_ref_unchecked(&self) -> &Self::Inner {
		&*self.get()
	}

	#[allow(clippy::mut_from_ref)] // That's the users' problem.
	unsafe fn get_mut_unchecked(&self) -> &mut Self::Inner {
		&mut *self.get()
	}
}

unsafe impl<T: ?Sized> UnsafeCellExt for UnsafeCell<T> {
	type Inner = T;

	fn get(&self) -> *mut Self::Inner {
		// This is shadowed by the inherent `impl`.
		self.get()
	}

	fn get_mut(&mut self) -> &mut Self::Inner {
		self.get_mut()
	}
}

// === MutexedUnsafeCell === //

/// A type of [UnsafeCell] that asserts that the given cell will only be accessed by one thread at a
/// given time.
#[derive(Default, TransparentWrapper)]
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

unsafe impl<T: ?Sized> UnsafeCellExt for MutexedUnsafeCell<T> {
	type Inner = T;

	fn get(&self) -> *mut Self::Inner {
		self.0.get()
	}
}

// impl<T, U> CoerceUnsized<MutexedUnsafeCell<U>> for MutexedUnsafeCell<T> where T: CoerceUnsized<U> {}
