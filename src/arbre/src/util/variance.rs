use std::marker::PhantomData;

macro derive_phantom($target:ident, $new:ident) {
    impl<T: ?Sized> $target<T> {
        pub const fn $new() -> Self {
            Self (PhantomData)
        }
    }

    impl<T: ?Sized> Copy for $target<T> {}
    impl<T: ?Sized> Clone for $target<T> {
        fn clone(&self) -> Self {
            Self (PhantomData)
        }
    }

    impl<T: ?Sized> Default for $target<T> {
        fn default() -> Self {
            Self (PhantomData)
        }
    }

    unsafe impl<T: ?Sized> Send for $target<T> {}
    unsafe impl<T: ?Sized> Sync for $target<T> {}
}

pub struct PhantomInvariant<T: ?Sized>(PhantomData<*mut T>);
pub struct PhantomCovariant<T: ?Sized>(PhantomData<*const T>);
// pub struct PhantomContravariant<T: ?Sized>(PhantomData<fn(T)>);

derive_phantom!(PhantomInvariant, new);
derive_phantom!(PhantomCovariant, new);
// derive_phantom!(PhantomContravariant, new);
