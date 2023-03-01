use std::{cell::UnsafeCell, marker::PhantomData};

/// A phantom marker that makes the structure invariant w.r.t `T`'s lifetime. In other words, `T`'s
/// lifetime can be neither extended nor shrunk.
pub type PhantomInvariant<T> = PhantomData<fn(T) -> T>;

/// A phantom marker that makes the structure contravariant w.r.t `T`'s lifetime. In other words,
/// `T`'s lifetime can be prolonged but not shrunk.
pub type PhantomProlong<T> = PhantomData<fn(T)>;

/// A phantom marker that makes the structure covariant w.r.t `T`'s lifetime. In other words, `T`'s
/// lifetime can be shrunk but not prolonged.
pub type PhantomShorten<T> = PhantomData<fn() -> T>;

/// A phantom marker that makes the structure neither [Send] nor [Sync].
pub type PhantomNoSendOrSync = PhantomData<*mut ()>;

/// A phantom marker that makes the structure non-[Sync] but implies nothing about [Send].
pub type PhantomNoSync = PhantomData<UnsafeCell<()>>;

/// A phantom marker that makes the structure non-[Send]  but implies nothing about [Sync].
pub type PhantomNoSend = PhantomData<sealed::NoSendOnly>;

mod sealed {
	pub struct NoSendOnly(super::PhantomNoSendOrSync);

	unsafe impl Sync for NoSendOnly {}
}
