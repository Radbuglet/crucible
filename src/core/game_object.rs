// TODO: Docs, safety, clean up

use std::any::TypeId;
use std::hash;
use std::marker::PhantomData;
use std::mem::MaybeUninit;

// === Keys === //

#[derive(Hash, Eq, PartialEq, Copy, Clone)]
struct RawKey(TypeId);

pub struct Key<T: ?Sized> {
    // Parameter lifetime is invariant because users could potentially provide keys with an insufficient
    // lifetime.
    _ty: PhantomData<std::cell::Cell<T>>,

    /// The program unique identifier of the key.
    raw_id: RawKey,
}

impl<T: ?Sized> Key<T> {
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

struct KeyTargetInner {
    is_init: bool,
    raw_key: RawKey,
}

struct KeyTarget<'val, T: ?Sized> {
    inner: KeyTargetInner,
    out_val: MaybeUninit<&'val T>,
}

impl<'val, T: ?Sized> KeyTarget<'val, T> {
    pub fn new(key: Key<T>) -> Self {
        Self {
            inner: KeyTargetInner {
                is_init: false,
                raw_key: key.raw_id,
            },
            out_val: MaybeUninit::uninit(),
        }
    }

    pub fn is_init(&self) -> bool {
        self.inner.is_init
    }

    pub fn get(&self) -> Option<&'val T> {
        if self.is_init() {
            Some (unsafe { self.out_val.assume_init() })
        } else {
            None
        }
    }

    pub fn out_view<'view>(&'view mut self) -> KeyOut<'view, 'val> {
        KeyOut {
            inner: &mut self.inner,
            ptr_ty: PhantomData,
            ptr: self.out_val.as_mut_ptr().cast::<()>(),
        }
    }
}

pub struct KeyOut<'view, 'val> {
    inner: &'view mut KeyTargetInner,

    // `'view` is covariant, `'val` is invariant.
    ptr_ty: PhantomData<&'view mut &'val ()>,
    ptr: *mut (),
}

impl<'view, 'val> KeyOut<'view, 'val> {
    pub fn is_init(&self) -> bool {
        self.inner.is_init
    }

    pub fn try_put_field<T: ?Sized>(&mut self, field_key: Key<T>, field_ref: &'val T) -> bool {
        debug_assert!(!self.is_init());

        if field_key.raw_id == self.inner.raw_key {
            unsafe {
                self.ptr.cast::<&'val T>().write(field_ref);
            }

            self.inner.is_init = true;
            true
        } else {
            false
        }
    }
}

pub trait GameObject {
    fn get_raw(&self, out: &mut KeyOut) -> bool;
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
        let mut target = KeyTarget::<'a, T>::new(key);
        self.get_raw(&mut target.out_view());
        target.get()
    }
}
