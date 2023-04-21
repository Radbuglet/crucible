use std::{cell::Cell, ops::Deref, process::abort, ptr::NonNull};

// === Transformations === //

// When working with lifetime escape-hatches, we are better off being painfully explicit than
// being [explicit] painfully.
#[allow(clippy::needless_lifetimes)]
pub fn try_transform_mut<'a, T: ?Sized, R: ?Sized, F>(
	orig: &'a mut T,
	f: F,
) -> Result<&'a mut R, &'a mut T>
where
	F: FnOnce(&mut T) -> Option<&mut R>,
{
	let orig_ptr = orig as *mut T;

	match f(orig) {
		Some(new) => Ok(new),
		None => Err(unsafe {
			// Safety: `f` can only hold onto `orig` for the duration of the function invocation.
			// Since nothing has been returned, we have exclusive access to `orig_ptr` and can acquire
			// a mutable reference to it as desired.
			&mut *orig_ptr
		}),
	}
}

#[allow(clippy::needless_lifetimes)]
pub fn try_transform_ref<'a, T: ?Sized, R: ?Sized, F>(
	orig: &'a mut T,
	f: F,
) -> Result<&'a R, &'a mut T>
where
	F: FnOnce(&mut T) -> Option<&R>,
{
	let orig_ptr = orig as *mut T;

	match f(orig) {
		Some(new) => Ok(new),
		None => Err(unsafe {
			// Safety: see `try_transform`
			&mut *orig_ptr
		}),
	}
}

#[allow(clippy::needless_lifetimes)]
pub fn try_transform_mut_or_err<T, R, E, F>(orig: &mut T, f: F) -> Result<&mut R, (&mut T, E)>
where
	T: ?Sized,
	R: ?Sized,
	F: FnOnce(&mut T) -> Result<&mut R, E>,
{
	let mut err_reg = None;

	match try_transform_mut(orig, |lent| match f(lent) {
		Ok(xformed) => Some(xformed),
		Err(err) => {
			err_reg = Some(err);
			None
		}
	}) {
		Ok(xformed) => Ok(xformed),
		Err(orig) => Err((orig, err_reg.unwrap())),
	}
}

// === dynamic_value === //

pub struct Dynamic<T: ?Sized> {
	refs: Cell<usize>,
	value: T,
}

impl<T: ?Sized> Dynamic<T> {
	pub fn new(value: T) -> Self
	where
		T: Sized,
	{
		Self {
			refs: Cell::new(0),
			value,
		}
	}

	fn inc_refs(&self) {
		self.refs.set(self.refs.get().checked_add(1).unwrap());
	}

	fn dec_refs(&self) {
		self.refs.set(self.refs.get() - 1);
	}
}

impl<T: ?Sized> Drop for Dynamic<T> {
	fn drop(&mut self) {
		if self.refs.get() > 0 {
			abort();
		}
	}
}

pub struct DynamicRef<T: ?Sized> {
	// Invariants:
	//
	// a) The pointee will not be mutably reborrowed until *its* destructor is ran.
	// b) This pointer will be valid for as long as this `DynamicRef` instance is accessible.
	//
	value: NonNull<Dynamic<T>>,
}

impl<T: ?Sized> DynamicRef<T> {
	pub unsafe fn new(value: &Dynamic<T>) -> Self {
		value.inc_refs();

		// Safety: The caller guarantees that `&Dynamic` will not be reborrowed mutably until *its*
		// destructor is run, satisfying condition A. Because `Dynamic`'s destructor aborts the
		// program if any `DynamicRefs` to it remain, thereby making `Self` inaccessible, we know
		// invariant condition B is satisfied as well since no one can either drop or reborrow the
		// pointer while we are accessible.
		Self {
			value: NonNull::from(value),
		}
	}
}

impl<T: ?Sized> Clone for DynamicRef<T> {
	fn clone(&self) -> Self {
		unsafe {
			// Safety: provided by structure invariant A.
			Self::new(self.value.as_ref())
		}
	}
}

impl<T: ?Sized> Deref for DynamicRef<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		unsafe {
			// Safety: provided by structure invariant B.
			&self.value.as_ref().value
		}
	}
}

impl<T: ?Sized> Drop for DynamicRef<T> {
	fn drop(&mut self) {
		unsafe {
			// Safety: provided by structure invariant B.
			self.value.as_ref().dec_refs();
		};
	}
}

#[macro_export]
macro_rules! dynamic_value {
	($(let $name:ident$(: $ty:ty)? = $expr:expr;)*) => {$(
		let $name$(: &$crate::lang::lifetime::Dynamic<$ty>)? = &$crate::lang::lifetime::Dynamic::new($expr);
		let $name = unsafe {
			// Safety: `$name` isn't nameable so it certainly cannot be reborrowed until the function
			// ends, at which point its destructor will run by itself.
			$crate::lang::lifetime::DynamicRef::new($name)
		};
	)*};
}

pub use dynamic_value;
