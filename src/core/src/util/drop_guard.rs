use std::{
	mem::{self, ManuallyDrop},
	ops::{Deref, DerefMut},
};

#[derive(Debug)]
pub struct DropGuard<T, F>
where
	F: DropGuardHandler<T>,
{
	inner: ManuallyDrop<GuardInner<T, F>>,
}

#[derive(Debug)]
struct GuardInner<T, F> {
	target: T,
	handler: F,
}

impl<T, F> DropGuard<T, F>
where
	F: DropGuardHandler<T>,
{
	pub fn new(target: T, handler: F) -> Self {
		Self {
			inner: ManuallyDrop::new(GuardInner { target, handler }),
		}
	}

	pub fn defuse(mut me: Self) -> T {
		let inner = unsafe { ManuallyDrop::take(&mut me.inner) };
		mem::forget(me);
		inner.target
	}
}

impl<T, F> Deref for DropGuard<T, F>
where
	F: DropGuardHandler<T>,
{
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.inner.target
	}
}

impl<T, F> DerefMut for DropGuard<T, F>
where
	F: DropGuardHandler<T>,
{
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.inner.target
	}
}

impl<T, F> Drop for DropGuard<T, F>
where
	F: DropGuardHandler<T>,
{
	fn drop(&mut self) {
		let inner = unsafe { ManuallyDrop::take(&mut self.inner) };
		inner.handler.destruct(inner.target);
	}
}

pub trait DropGuardHandler<T> {
	fn destruct(self, value: T);
}

impl<T, F> DropGuardHandler<T> for F
where
	F: FnOnce(T),
{
	fn destruct(self, value: T) {
		(self)(value)
	}
}
