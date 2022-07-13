use std::{cell::UnsafeCell, marker::PhantomData};

pub type PhantomNoSendOrSync = PhantomData<*mut ()>;
pub type PhantomNoSync = PhantomData<UnsafeCell<()>>;
pub type PhantomNoSend = PhantomData<NoSendOnly>;

pub type PhantomInvariant<T> = PhantomData<fn(T) -> T>;
pub type PhantomProlong<T> = PhantomData<fn(T)>;
pub type PhantomShorten<T> = PhantomData<fn() -> T>;

pub struct NoSendOnly {
	_neither: PhantomNoSendOrSync,
}

unsafe impl Sync for NoSendOnly {}
