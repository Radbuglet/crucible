use std::{cell::UnsafeCell, marker::PhantomData};

pub type PhantomNoSendOrSync = PhantomData<*mut ()>;
pub type PhantomNoSync = PhantomData<UnsafeCell<()>>;
pub type PhantomNoSend = PhantomData<NoSendOnly>;

pub struct NoSendOnly {
	_neither: PhantomNoSendOrSync,
}

unsafe impl Sync for NoSendOnly {}
