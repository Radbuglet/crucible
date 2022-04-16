use std::fmt::{Debug, Formatter};
use std::hash::{Hash, Hasher};
use std::mem::MaybeUninit;

/// An object that's usually initialized... except when it isn't.
///
/// Specifically, the value may be temporarily uninitialized during some internal invariant-breaking
/// procedures but the slot is assumed to be initialized during the rest of the time and thus,
///
/// - The destructor assumes that the inner value is initialized.
/// - Users can safely access the contents, even if the value is actually uninitialized.
///
/// In other words, we're emulating intuitive C-style "this value may be uninitialized but we'll
/// still assume that it is initialized if you try and access it" behavior.
///
/// Good luck, code review.
pub struct UsuallyInit<T>(MaybeUninit<T>);

impl<T> UsuallyInit<T> {
    pub fn new(value: T) -> Self {
        Self(MaybeUninit::new(value))
    }

    pub unsafe fn uninit() -> Self {
        Self(MaybeUninit::uninit())
    }

    pub fn as_ptr(&self) -> *const T {
        self.0.as_ptr()
    }

    pub fn as_mut_ptr(&mut self) -> *mut T {
        self.0.as_mut_ptr()
    }

    pub fn as_ref(&self) -> &T {
        unsafe { self.0.assume_init_ref() }
    }

    pub fn as_mut(&mut self) -> &mut T {
        unsafe { self.0.assume_init_mut() }
    }

    pub unsafe fn read(&self) -> T {
        self.0.assume_init_read()
    }

    pub fn write(&mut self, value: T) {
        self.0.write(value);
    }

    pub fn unwrap(self) -> T {
        unsafe { self.0.assume_init_read() }
    }
}

impl<T: Debug> Debug for UsuallyInit<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.as_ref().fmt(f)
    }
}

impl<T: Clone> Clone for UsuallyInit<T> {
    fn clone(&self) -> Self {
        Self::new(self.as_ref().clone())
    }
}

impl<T: Hash> Hash for UsuallyInit<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_ref().hash(state);
    }
}

impl<T: Eq> Eq for UsuallyInit<T> {}

impl<T: PartialEq> PartialEq for UsuallyInit<T> {
    fn eq(&self, other: &Self) -> bool {
        self.as_ref().eq(other.as_ref())
    }
}

impl<T> Drop for UsuallyInit<T> {
    fn drop(&mut self) {
        unsafe { self.0.assume_init_drop() }
    }
}
