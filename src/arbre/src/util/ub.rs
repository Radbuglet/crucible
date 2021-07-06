//! Wrappers around unsafe methods that report (some forms of) illegal usage in debug builds.

#![allow(dead_code)]

use std::mem;
use std::ops::{Deref, DerefMut};
use std::ptr::NonNull;

// === Checked method variants === //

#[cfg(debug_assertions)]
pub unsafe fn unreachable_unchecked() -> ! {
    unreachable!()
}
#[cfg(not(debug_assertions))]
pub use std::hint::unreachable_unchecked;

pub unsafe fn unchecked_index_mut<T>(collection: &mut [T], index: usize) -> &mut T {
    debug_assert!(index < collection.len(), "Invalid index in call to `unchecked_index`");
    collection.get_unchecked_mut(index)
}

pub unsafe fn unwrap_unchecked<T>(option: Option<T>) -> T {
    match option {
        Some(value) => value,
        None => unreachable_unchecked(),
    }
}

pub unsafe fn new_non_null<T>(ptr: *mut T) -> NonNull<T> {
    debug_assert!(!ptr.is_null());

    NonNull::new_unchecked(ptr)
}

pub unsafe fn offset_ptr<T>(ptr: NonNull<T>, count: isize) -> NonNull<T> {
    debug_assert!(
        count.checked_mul(mem::size_of::<T>() as isize).is_some(),
        "\"count\" in bytes overflowed the `isize`"
    );

    new_non_null(ptr.as_ptr().offset(count))
}

pub unsafe fn add_ptr<T>(ptr: NonNull<T>, count: usize) -> NonNull<T> {
    debug_assert!(count <= isize::MAX as usize);

    offset_ptr(ptr, count as isize)
}

pub unsafe fn sub_ptr<T>(ptr: NonNull<T>, count: usize) -> NonNull<T> {
    debug_assert!(count <= isize::MAX as usize);

    offset_ptr(ptr, -(count as isize))
}

// === ManualCell === //

#[cfg(debug_assertions)]
use std::cell::{RefCell, Ref, RefMut};

#[cfg(not(debug_assertions))]
use std::cell::UnsafeCell;

/// A version of `UnsafeCell` that is checked in debug builds.
pub struct ManualCell<T: ?Sized> {
    #[cfg(debug_assertions)]
    cell: RefCell<T>,

    #[cfg(not(debug_assertions))]
    // Prevents Send + Sync, provides proper (in)variance and drop-check information.
    cell: UnsafeCell<T>,
}

impl<T: Default> Default for ManualCell<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T> From<T> for ManualCell<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl<T> ManualCell<T> {
    pub const fn new(value: T) -> Self {
        Self {
            #[cfg(debug_assertions)]
            cell: RefCell::new(value),

            #[cfg(not(debug_assertions))]
            cell: UnsafeCell::new(value),
        }
    }
}

impl<T: ?Sized> ManualCell<T> {
    #[inline]
    pub unsafe fn borrow(&self) -> ManualRef<T> {
        ManualRef {
            #[cfg(debug_assertions)]
            ptr: self.cell.borrow(),

            #[cfg(not(debug_assertions))]
            // Safety: provided by caller
            ptr: &*self.cell.get()
        }
    }

    #[inline]
    pub unsafe fn borrow_mut(&self) -> ManualRefMut<T> {
        ManualRefMut {
            #[cfg(debug_assertions)]
            ptr: self.cell.borrow_mut(),

            #[cfg(not(debug_assertions))]
            // Safety: provided by caller
            ptr: &mut *self.cell.get()
        }
    }

    pub fn get_mut(&mut self) -> &mut T {
        // This method has the same name and signature for both `UnsafeCell` and `RefCell`.
        self.cell.get_mut()
    }
}

pub struct ManualRef<'a, T: ?Sized> {
    #[cfg(debug_assertions)]
    ptr: Ref<'a, T>,

    #[cfg(not(debug_assertions))]
    ptr: &'a T,
}

impl<T: ?Sized> Deref for ManualRef<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &*self.ptr
    }
}

pub struct ManualRefMut<'a, T: ?Sized> {
    #[cfg(debug_assertions)]
    ptr: RefMut<'a, T>,

    #[cfg(not(debug_assertions))]
    ptr: &'a mut T,
}

impl<T: ?Sized> Deref for ManualRefMut<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &*self.ptr
    }
}

impl<T: ?Sized> DerefMut for ManualRefMut<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut *self.ptr
    }
}
