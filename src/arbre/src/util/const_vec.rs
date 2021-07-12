use std::mem::MaybeUninit;
use std::ptr::{slice_from_raw_parts, slice_from_raw_parts_mut};

pub struct ConstVec<T, const CAP: usize> {
    array: [MaybeUninit<T>; CAP],
    len: usize,
}

impl<T: Copy, const CAP: usize> ConstVec<T, { CAP }> {
    pub const fn new() -> Self {
        Self {
            array: [MaybeUninit::<T>::uninit(); CAP],
            len: 0,
        }
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn try_push(&mut self, elem: T) -> bool {
        if self.len != CAP {
            self.array[self.len] = MaybeUninit::new(elem);
            self.len += 1;
            true
        } else {
            false
        }
    }

    pub const fn push(&mut self, elem: T) {
        if !self.try_push(elem) {
            panic!("Cannot push element: `ConstVec` would grow past its capacity.");
        }
    }

    pub const fn pop(&mut self) {
        if self.len == 0 {
            panic!("Cannot pop a `ConstVec` with zero elements.");
        }
        self.len -= 1;
    }

    pub const fn get(&self, index: usize) -> &T {
        if index >= self.len {
            panic!("Index out of bounds.");
        }
        unsafe { self.array[index].assume_init_ref() }
    }

    pub const fn get_mut(&mut self, index: usize) -> &mut T {
        if index >= self.len {
            panic!("Index out of bounds.");
        }
        unsafe { self.array[index].assume_init_mut() }
    }

    pub const fn swap_remove(&mut self, removed: usize) {
        self.array[removed] = self.array[self.len - 1];
        self.pop();
    }

    pub const fn as_slice(&self) -> &[T] {
        unsafe {
            // Safety:
            // - `array`'s root is a valid pointer.
            // - `MaybeUninit<T>` is `#[repr(transparent)]`, allowing us to reinterpret the array pointer
            //    as `*const T` for indices `0..self.len`.
            // - self.len is less than or equal to the length of the backing array.
            // - The slice will live as long as `'_`.
            &*slice_from_raw_parts(
                self.array.as_ptr().cast::<T>(),
                self.len,
            )
        }
    }

    pub const fn as_slice_mut(&mut self) -> &mut [T] {
        unsafe {
            // Safety: See `as_slice`.
            &mut *slice_from_raw_parts_mut(
                self.array.as_mut_ptr().cast::<T>(),
                self.len,
            )
        }
    }

    pub const fn clone(&self) -> Self {
        let mut other = ConstVec::new();
        let mut index = 0;
        while index < self.len() {
            other.push(*self.get(index));
            index += 1;
        }
        other
    }
}
