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
    /// lifetime. The effect on dropck (which tells it that we can access instances of `T` during `Drop`)
    /// doesn't matter.
    _ty: PhantomData<std::cell::Cell<T>>,

    /// The program unique identifier of the key.
    raw_id: RawKey,
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
    pub const unsafe fn new<K: 'static>() -> Self {
        Self {
            _ty: PhantomData,
            raw_id: RawKey (TypeId::of::<K>()),
        }
    }
}

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
        Key::<$type>::new::<UniqueTy>()
    }
}

// === Game Objects === //

pub struct KeyOut<'view, 'val> {
    ptr_ty: PhantomData<&'view mut &'val ()>,
    ptr: *mut (),
    is_init: bool,
    raw_key: RawKey,
}

impl<'view, 'val> KeyOut<'view, 'val> {
    fn new<T: ?Sized>(key: Key<T>, target: &'view mut MaybeUninit<&'val T>) -> Self {
        Self {
            ptr_ty: PhantomData,
            ptr: target.as_mut_ptr().cast::<()>(),
            is_init: false,
            raw_key: key.raw_id,
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

pub trait GameObject {
    // Note: the returned value *cannot* be relied upon for correctness and solely exists for the user's
    // convenience while composing `get_raw` calls. Internal code must use [KeyOut::is_init] instead.
    fn get_raw<'val>(&'val self, out: &mut KeyOut<'_, 'val>) -> bool;
}

pub trait GameObjectExt {
    fn get<T: ?Sized>(&self, key: Key<T>) -> &T;
    fn has<T: ?Sized>(&self, key: Key<T>) -> bool;
    fn try_get<T: ?Sized>(&self, key: Key<T>) -> Option<&T>;
}

impl<B: ?Sized + GameObject> GameObjectExt for B {
    fn get<T: ?Sized>(&self, key: Key<T>) -> &T {
        self.try_get(key).unwrap()
    }

    fn has<T: ?Sized>(&self, key: Key<T>) -> bool {
        self.try_get(key).is_some()
    }

    fn try_get<'a, T: ?Sized>(&'a self, key: Key<T>) -> Option<&'a T> {
        let mut target = MaybeUninit::<&'a T>::uninit();
        let mut view = KeyOut::new(key, &mut target);

        self.get_raw(&mut view);

        if view.is_init() {
            Some (unsafe { target.assume_init() })
        } else {
            None
        }
    }
}
