use core::{cell::UnsafeCell, marker::PhantomData};

pub type PhantomInvariant<T> = PhantomData<fn(T) -> T>;
pub type PhantomIn<T> = PhantomData<fn(T)>;
pub type PhantomOut<T> = PhantomData<fn() -> T>;

pub type PhantomNoSendOrSync = PhantomData<*mut ()>;
pub type PhantomNoSync = PhantomData<UnsafeCell<()>>;
pub type PhantomNoSend = PhantomData<NoSendOnly>;

pub struct NoSendOnly {
	_neither: PhantomNoSendOrSync,
}

unsafe impl Sync for NoSendOnly {}
