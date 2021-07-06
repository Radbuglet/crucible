// TODO: Implement new v-table
// TODO: Finish docs, tests

use std::hash;
use std::intrinsics::type_id;
use std::marker::PhantomData;

// === Keys === //

/// An untyped program unique identifier underlying [Key].
#[derive(Hash, Eq, PartialEq, Copy, Clone)]
pub struct RawKey(u64);

impl RawKey {
    pub const fn new<T: ?Sized + 'static>() -> Self {
        Self (type_id::<T>())
    }

    pub const fn id(self) -> u64 {
        self.0
    }
}

/// A unique identifier for a component of type `T`.
pub struct Key<T: ?Sized> {
    /// Marker to bind the `T` generic parameter.
    ///
    /// Parameter lifetime is invariant because users could potentially provide keys with an insufficient
    /// lifetime. Has no effect on dropck (we only tell it that we can access the pointer, but not the
    /// instances of `T`, during `Drop`).
    _ty: PhantomData<*mut T>,

    /// The program unique identifier of the key.
    raw_id: RawKey,
}

impl<T: ?Sized + 'static> Key<T> {
    /// Constructs the singleton key for the specified component type. As per `std::any::TypeId`'s
    /// limitations, this type must live for `'static`.
    pub const fn typed() -> Self {
        Self {
            _ty: PhantomData,
            raw_id: RawKey::new::<T>()
        }
    }
}

impl<T: ?Sized> Key<T> {
    /// An *internal method* to create a new `Key` using the type `K` as an identifier provider. Use
    /// the [new_key] macro to automate this process.
    ///
    /// ## Safety
    ///
    /// `K` must only ever be associated with a single type `T` and failing to do so will cause
    /// unsoundness in the type system.
    ///
    #[doc(hidden)]
    pub const unsafe fn new_arbitrary<K: 'static>() -> Self {
        Self {
            _ty: PhantomData,
            raw_id: RawKey::new::<K>(),
        }
    }
}

// Because the `*mut T` pointer prevents the auto impl from deriving it for us.
// Safety: we only rely on this struct's id, which is guaranteed to be unique at compile time--unaffected
// by multithreading.
unsafe impl<T: ?Sized> Send for Key<T> {}
unsafe impl<T: ?Sized> Sync for Key<T> {}

impl<T: ?Sized> hash::Hash for Key<T> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.raw_id.hash(state)
    }
}

impl<T: ?Sized> Eq for Key<T> {}
impl<T: ?Sized> PartialEq for Key<T> {
    fn eq(&self, other: &Self) -> bool {
        self.raw_id.eq(&other.raw_id)
    }
}

impl<T: ?Sized> Copy for Key<T> {}
impl<T: ?Sized> Clone for Key<T> {
    fn clone(&self) -> Self {
        Self {
            _ty: PhantomData,
            raw_id: self.raw_id,
        }
    }
}

/// Constructs a brand new `Key` to a component of the specified type.
///
/// ## Syntax
///
/// ```no_run
/// new_key!(
///     $type:ty  // The type of the component
/// );
/// ```
///
pub macro new_key($type:ty) {
    unsafe {
        struct UniqueTy;

        // Safety: `UniqueTy` is guaranteed to be a unique type.
        Key::<$type>::new_arbitrary::<UniqueTy>()
    }
}

// === Provider definition === //

pub unsafe trait Provider {
    // TODO: Migrate to associated variable constants once available.
    fn table(&self) -> ();
}

pub macro provide(  // TODO: Generics
    $target:ty
    $([$(.$field:ident),*])?
    $(=> $($type:ty),*)?
) {

}

// === Extension methods === //

// pub struct Comp<O, T> {
//     pub obj: O,
//     pub comp: T,
// }
//
// // TODO: Coercions
//
// impl<O, T> Comp<O, T> {
//     pub fn new(obj: O, comp: T) -> Self {
//         Self { obj, comp }
//     }
// }
//
// impl<O, T> Deref for Comp<O, T> {
//     type Target = T;
//
//     fn deref(&self) -> &Self::Target {
//         &self.comp
//     }
// }
//
// pub trait ProviderExt {
//     fn try_fetch_key<S: Deref<Target = Self>, T: ?Sized>(self: S, key: Key<T>) -> Option<Comp<S, &T>>;
//
//     fn fetch_key<T: ?Sized>(&self, key: Key<T>) -> Comp<Self::Obj, &T> {
//         self.try_fetch_key(key).unwrap()
//     }
//
//     fn has_key<T: ?Sized>(&self, key: Key<T>) -> bool {
//         self.try_fetch_key(key).is_some()
//     }
//
//     fn try_fetch<T: ?Sized + 'static>(&self) -> Option<Comp<Self::Obj, T>> {
//         self.try_fetch_key(Key::<T>::typed())
//     }
//
//     fn fetch<T: ?Sized + 'static>(&self) -> Comp<Self::Obj, T> {
//         self.fetch_key(Key::<T>::typed())
//     }
//
//     fn has<T: ?Sized + 'static>(&self) -> bool {
//         self.has_key(Key::<T>::typed())
//     }
// }
//
// impl<B: ?Sized + Provider> ProviderExt for B {
//     type Obj = ();
//
//     fn try_fetch_key<T: ?Sized>(&self, key: Key<T>) -> Option<Comp<Self::Obj, &T>> {
//         unimplemented!()
//     }
// }
