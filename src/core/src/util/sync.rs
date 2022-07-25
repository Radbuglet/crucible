use core::{any, fmt};

#[derive(Default)]
pub struct AssertSync<T: ?Sized>(T);

// Safety: users can only unwrap references to `AssertSync` via the unsafe `AssertSync::get` method.
unsafe impl<T: ?Sized> Sync for AssertSync<T> {}

impl<T: ?Sized> fmt::Debug for AssertSync<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct(format!("AssertSync<{}>", any::type_name::<T>()).as_str())
			.finish_non_exhaustive()
	}
}

impl<T> AssertSync<T> {
	pub const fn new(value: T) -> Self {
		Self(value)
	}

	pub fn into_inner(self) -> T {
		self.0
	}
}

impl<T: ?Sized> AssertSync<T> {
	pub unsafe fn get(&self) -> &T {
		// Safety: provided by caller
		&self.0
	}
}

impl<T: ?Sized + Send> AssertSync<T> {
	pub fn get_mut(&mut self) -> &mut T {
		// Safety: `&mut T: Send` so long as `T: Send`.
		&mut self.0
	}
}

// impl<T, U> CoerceUnsized<OnlyMut<U>> for OnlyMut<T> where T: CoerceUnsized<U> {}
