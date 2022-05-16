use std::cell::UnsafeCell;
use std::fmt::{Debug, Formatter};
use std::marker::PhantomData;

#[derive(Default, Copy, Clone)]
pub struct PhantomNoSync {
	_ty: PhantomData<UnsafeCell<()>>,
}

impl Debug for PhantomNoSync {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		f.debug_tuple("PhantomNoSync").finish()
	}
}

#[derive(Default, Copy, Clone)]
pub struct PhantomNoSendOrSync {
	_ty: PhantomData<*const ()>,
}

impl Debug for PhantomNoSendOrSync {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		f.debug_tuple("PhantomNoSend").finish()
	}
}
