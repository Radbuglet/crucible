use std::cell::UnsafeCell;

macro_rules! define_unsafe_cell {
    ($(
        $(#[$attr:meta])*
        $name:ident;
    )*) => {$(
        $(#[$attr])*
        #[repr(transparent)]
        pub struct $name<T: ?Sized> {
            value: UnsafeCell<T>,
        }

        impl<T> $name<T> {
            #[inline(always)]
            pub const fn new(value: T) -> Self {
                Self { value: UnsafeCell::new(value) }
            }

            #[inline(always)]
            pub fn into_inner(self) -> T {
                self.value.into_inner()
            }
        }

        impl<T: ?Sized> $name<T> {
            pub fn from_mut(value: &mut T) -> &mut Self {
                unsafe { &mut *(value as *mut T as *mut Self) }
            }

            pub const fn get(&self) -> *mut T {
                self.value.get()
            }

            pub fn get_mut(&mut self) -> &mut T {
                self.value.get_mut()
            }

            pub const fn raw_get(this: *const Self) -> *mut T {
                UnsafeCell::raw_get(this as *const Self as *const UnsafeCell<T>)
            }
        }

        impl<T: Default> Default for $name<T> {
            fn default() -> Self {
                Self::new(T::default())
            }
        }

        impl<T> From<T> for $name<T> {
            fn from(value: T) -> Self {
                Self::new(value)
            }
        }
    )*};
}

define_unsafe_cell! {
    SyncUnsafeCell;
    WoUnsafeCell;
}

unsafe impl<T: ?Sized + Send + Sync> Sync for SyncUnsafeCell<T> {}

unsafe impl<T: ?Sized + Send> Sync for WoUnsafeCell<T> {}
