use std::mem::MaybeUninit;

pub struct ConstVec<T, const CAP: usize> {
    array: [MaybeUninit<T>; CAP],
    len: usize,
}

impl<T, const CAP: usize> ConstVec<T, { CAP }> {
    const UNINIT_ELEM: MaybeUninit<T> = MaybeUninit::<T>::uninit();

    pub const fn new() -> Self {
        Self {
            array: [Self::UNINIT_ELEM; CAP],
            len: 0,
        }
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn push(&mut self, elem: T) {
        if self.len == CAP {
            panic!("Cannot push element: `ConstVec` would grow past its capacity.")
        }
        self.array[self.len] = MaybeUninit::new(elem);
        self.len += 1;
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
}
