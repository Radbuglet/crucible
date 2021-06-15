// TODO: Context preservation
// TODO: Non-ref components
// TODO: Enable better compile-time borrow checking
// TODO: Docs (internal and external) and tests

use std::any::TypeId;
use std::hash;
use std::marker::PhantomData;
use std::mem::MaybeUninit;

// === Keys === //

/// The untyped program unique identifier underlying [Key].
#[derive(Hash, Eq, PartialEq, Copy, Clone)]
struct RawKey(TypeId);

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
    pub const fn typed() -> Self {
        Self {
            _ty: PhantomData,
            raw_id: RawKey (TypeId::of::<T>())
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
            raw_id: RawKey (TypeId::of::<K>()),
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

pub macro new_key($type:ty) {
    unsafe {
        struct UniqueTy;

        // Safety: `UniqueTy` is guaranteed to be a unique type.
        Key::<$type>::new_arbitrary::<UniqueTy>()
    }
}

// === Game Object Core === //

pub trait GameObject {
    // Note: the returned value *cannot* be relied upon for correctness and solely exists for the user's
    // convenience while composing `get_raw` calls. Internal code must use [KeyOut::is_init] instead.
    fn get_raw<'val>(&'val self, out: &mut KeyOut<'_, 'val>) -> bool;
}

impl<T: 'static> GameObject for T {
    default fn get_raw<'val>(&'val self, out: &mut KeyOut<'_, 'val>) -> bool {
        out.try_put_field(Key::typed(), self)
    }
}

impl<T: ?Sized + GameObject> GameObject for &'_ T {
    fn get_raw<'val>(&'val self, out: &mut KeyOut<'_, 'val>) -> bool {
        (*self).get_raw(out)
    }
}

impl<T: ?Sized + GameObject> GameObject for &'_ mut T {
    fn get_raw<'val>(&'val self, out: &mut KeyOut<'_, 'val>) -> bool {
        (&**self).get_raw(out)
    }
}

pub struct KeyOut<'view, 'val> {
    ptr_ty: PhantomData<&'view mut &'val ()>,
    ptr: *mut (),
    raw_key: RawKey,
    is_init: bool,
}

impl<'view, 'val> KeyOut<'view, 'val> {
    fn new<T: ?Sized>(key: Key<T>, target: &'view mut MaybeUninit<&'val T>) -> Self {
        Self {
            ptr_ty: PhantomData,
            ptr: target.as_mut_ptr().cast::<()>(),
            raw_key: key.raw_id,
            is_init: false,
        }
    }

    pub fn is_init(&self) -> bool {
        self.is_init
    }

    pub fn try_put_field<T: ?Sized>(&mut self, field_key: Key<T>, field_ref: &'val T) -> bool {
        debug_assert!(!self.is_init);

        if field_key.raw_id == self.raw_key {
            unsafe {
                self.ptr.cast::<&'val T>().write(field_ref);
            }

            self.is_init = true;
            true
        } else {
            false
        }
    }
}

// === Game Object Extension Methods === //

pub trait GameObjectExt {
    fn try_fetch_key<T: ?Sized>(&self, key: Key<T>) -> Option<&T>;
    fn fetch_key<T: ?Sized>(&self, key: Key<T>) -> &T;
    fn has_key<T: ?Sized>(&self, key: Key<T>) -> bool;

    fn try_fetch<T: ?Sized + 'static>(&self) -> Option<&T>;
    fn fetch<T: ?Sized + 'static>(&self) -> &T;
    fn has<T: ?Sized + 'static>(&self) -> bool;

    fn try_fetch_many<'a, T: FetchMany<'a>>(&'a self) -> Option<T>;
    fn fetch_many<'a, T: FetchMany<'a>>(&'a self) -> T;
}

impl<B: ?Sized + GameObject> GameObjectExt for B {
    // Fetch by key ==
    fn try_fetch_key<'a, T: ?Sized>(&'a self, key: Key<T>) -> Option<&'a T> {
        let mut target = MaybeUninit::<&'a T>::uninit();
        let mut view = KeyOut::new(key, &mut target);

        self.get_raw(&mut view);

        if view.is_init() {
            Some (unsafe { target.assume_init() })
        } else {
            None
        }
    }

    fn fetch_key<T: ?Sized>(&self, key: Key<T>) -> &T {
        self.try_fetch_key(key).unwrap()
    }

    fn has_key<T: ?Sized>(&self, key: Key<T>) -> bool {
        self.try_fetch_key(key).is_some()
    }

    // Fetch by type ==
    fn try_fetch<T: ?Sized + 'static>(&self) -> Option<&T> {
        self.try_fetch_key(Key::<T>::typed())
    }

    fn fetch<T: ?Sized + 'static>(&self) -> &T {
        self.fetch_key(Key::<T>::typed())
    }

    fn has<T: ?Sized + 'static>(&self) -> bool {
        self.has_key(Key::<T>::typed())
    }

    // Fetch many ==
    fn try_fetch_many<'a, T: FetchMany<'a>>(&'a self) -> Option<T> {
        T::try_fetch_many(self)
    }

    fn fetch_many<'a, T: FetchMany<'a>>(&'a self) -> T {
        T::fetch_many(self)
    }
}

// === Game Object Tuple Utils === //

pub trait FetchMany<'a>: Sized {
    fn try_fetch_many<T: ?Sized + GameObject>(obj: &'a T) -> Option<Self>;

    fn fetch_many<T: ?Sized + GameObject>(obj: &'a T) -> Self {
        Self::try_fetch_many(obj).unwrap()
    }
}

// Constructs an expression guaranteed to return a tuple, regardless of the number of elements provided.
macro tup {
    // A special case to construct an empty tuple (`(,)` is illegal).
    () => { () },

    // For tuples with more than one element
    ($($elem:expr),+) => {
        (
            $ ($ elem),+
            ,  // A trailing comma forces the parser to treat the parens as a tuple and not an expression.
        )
    }
}

macro impl_tuple($($name:ident : $idx:tt),*) {
    // Fetching for `(&A, ..., &Z)`
    impl<'a, $($name: ?Sized + 'static),*> FetchMany<'a> for ($(&'a $name,)*) {
        #[allow(unused)]  // For empty tuples
        fn try_fetch_many<T: ?Sized + GameObject>(obj: &'a T) -> Option<Self> {
            Some (tup!(
                $(obj.try_fetch::<$name>()?),*
            ))
        }
    }

    // Providing for `(A, ..., Z)`
    impl<$($name: GameObject),*> GameObject for ($($name,)*) {
        #[allow(unused)]  // For empty tuples
        fn get_raw<'val>(&'val self, out: &mut KeyOut<'_, 'val>) -> bool {
            $(self.$idx.get_raw(out) ||)*
            false
        }
    }
}

impl_tuple!();
impl_tuple!(A:0);
impl_tuple!(A:0, B:1);
impl_tuple!(A:0, B:1, C:2);
impl_tuple!(A:0, B:1, C:2, D:3);
impl_tuple!(A:0, B:1, C:2, D:3, E: 4);
impl_tuple!(A:0, B:1, C:2, D:3, E: 4, F: 5);
impl_tuple!(A:0, B:1, C:2, D:3, E: 4, F: 5, G: 6);
impl_tuple!(A:0, B:1, C:2, D:3, E: 4, F: 5, G: 6, H: 7);
impl_tuple!(A:0, B:1, C:2, D:3, E: 4, F: 5, G: 6, H: 7, I: 8);
impl_tuple!(A:0, B:1, C:2, D:3, E: 4, F: 5, G: 6, H: 7, I: 8, J: 9);
impl_tuple!(A:0, B:1, C:2, D:3, E: 4, F: 5, G: 6, H: 7, I: 8, J: 9, K: 10);
impl_tuple!(A:0, B:1, C:2, D:3, E: 4, F: 5, G: 6, H: 7, I: 8, J: 9, K: 10, L: 11);
impl_tuple!(A:0, B:1, C:2, D:3, E: 4, F: 5, G: 6, H: 7, I: 8, J: 9, K: 10, L: 11, M: 12);
