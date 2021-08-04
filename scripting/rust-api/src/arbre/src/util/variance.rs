use std::marker::PhantomData;

pub type PhantomInvariant<T> = PhantomData<fn(T) -> T>;
pub type PhantomCovariant<T> = PhantomData<fn() -> T>;
pub type PhantomContravariant<T> = PhantomData<fn(T)>;
