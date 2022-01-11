pub unsafe trait Wrapper {
	type Underlying;

	// === Constructors === //

	fn from_raw(val: Self::Underlying) -> Self;

	fn from_ptr(ref_: *const Self::Underlying) -> *const Self;

	fn from_ptr_mut(ref_: *mut Self::Underlying) -> *mut Self;

	fn from_ref(ref_: &Self::Underlying) -> &Self {
		// Safety: pointers returned by `from_ptr` are guaranteed to live as long as the pointer
		// from which it was derived. They're the same pointer after all!
		unsafe { &*Self::from_ptr(ref_ as *const _) }
	}

	fn from_mut(ref_: &mut Self::Underlying) -> &mut Self {
		// Safety: pointers returned by `from_ptr_mut` are guaranteed to live as long as the pointer
		// from which it was derived. They're the same pointer after all!
		unsafe { &mut *Self::from_ptr_mut(ref_ as *mut _) }
	}

	// === Destructors === //

	fn to_raw(self) -> Self::Underlying;

	fn to_ptr(ptr: *const Self) -> *const Self::Underlying;

	fn to_ptr_mut(ptr: *mut Self) -> *mut Self::Underlying;

	fn to_ref(&self) -> &Self::Underlying {
		// Safety: pointers returned by `to_ptr` are guaranteed to live as long as the pointer
		// from which it was derived. They're the same pointer after all!
		unsafe { &*Self::to_ptr(self as *const _) }
	}

	fn to_mut(&mut self) -> &mut Self::Underlying {
		// Safety: pointers returned by `to_ptr_mut` are guaranteed to live as long as the pointer
		// from which it was derived. They're the same pointer after all!
		unsafe { &mut *Self::to_ptr_mut(self as *mut _) }
	}
}

pub macro new_wrapper(
    $(#[$item_attr:meta])*
    $vis:vis $name:ident;
) {
    $(#[$item_attr])*
    #[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, PartialOrd)]
    #[repr(transparent)]
    $vis struct $name<T>($vis T);

    unsafe impl<T> Wrapper for $name<T> {
        type Underlying = T;

        fn from_raw(val: Self::Underlying) -> Self {
            $name(val)
        }

        fn from_ptr(ref_: *const Self::Underlying) -> *const Self {
            // Safety: $name is repr(transparent) so pointers to its contents are identical to
            // pointers to its exterior. As such, both lifetime and validity requirements are met.
            ref_ as *const Self
        }

        fn from_ptr_mut(ref_: *mut Self::Underlying) -> *mut Self {
            // Safety: $name is repr(transparent) so pointers to its contents are identical to
            // pointers to its exterior. As such, both lifetime and validity requirements are met.
            ref_ as *mut Self
        }

        fn to_raw(self) -> Self::Underlying {
            self.0
        }

        fn to_ptr(ptr: *const Self) -> *const Self::Underlying {
            // Safety: $name is repr(transparent) so pointers to its contents are identical to
            // pointers to its exterior. As such, both lifetime and validity requirements are met.
            ptr as *const Self::Underlying
        }

        fn to_ptr_mut(ptr: *mut Self) -> *mut Self::Underlying {
            // Safety: $name is repr(transparent) so pointers to its contents are identical to
            // pointers to its exterior. As such, both lifetime and validity requirements are met.
            ptr as *mut Self::Underlying
        }
    }
}
