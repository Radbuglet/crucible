use std::{
    mem::{self, ManuallyDrop},
    ops::{Deref, DerefMut},
};

pub fn guard<T, F: FnOnce(T)>(value: T, func: F) -> DropGuard<T, F> {
    DropGuard::new(value, func)
}

pub fn defuse<T, F: DropGuardHandler<T>>(guard: DropGuard<T, F>) -> T {
    DropGuard::defuse(guard)
}

pub trait DropGuardHandler<T> {
    fn call(self, target: T);
}

impl<T, F: FnOnce(T)> DropGuardHandler<T> for F {
    fn call(self, target: T) {
        self(target)
    }
}

pub struct DropGuard<T, F: DropGuardHandler<T>> {
    value: ManuallyDrop<T>,
    func: ManuallyDrop<F>,
}

impl<T, F: DropGuardHandler<T>> DropGuard<T, F> {
    pub fn new(value: T, func: F) -> Self {
        Self {
            value: ManuallyDrop::new(value),
            func: ManuallyDrop::new(func),
        }
    }
    pub fn defuse_parts(mut me: Self) -> (T, F) {
        let value = unsafe { ManuallyDrop::take(&mut me.value) };
        let func = unsafe { ManuallyDrop::take(&mut me.func) };
        mem::forget(me);
        (value, func)
    }

    pub fn defuse(me: Self) -> T {
        Self::defuse_parts(me).0
    }

    pub fn defuse_func(me: Self) -> F {
        Self::defuse_parts(me).1
    }
}

impl<T, F: DropGuardHandler<T>> Deref for DropGuard<T, F> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T, F: DropGuardHandler<T>> DerefMut for DropGuard<T, F> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

impl<T, F: DropGuardHandler<T>> Drop for DropGuard<T, F> {
    fn drop(&mut self) {
        let value = unsafe { ManuallyDrop::take(&mut self.value) };
        let func = unsafe { ManuallyDrop::take(&mut self.func) };

        func.call(value);
    }
}
