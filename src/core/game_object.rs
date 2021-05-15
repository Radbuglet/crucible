// TODO: Docs, safety, and macro stuff
// TODO: Make `GameObjects` derivable manually without unsafe code.

use std::any::TypeId;
use std::hash;
use std::marker::PhantomData;
use std::mem::MaybeUninit;

// === Keys === //

#[derive(Hash, Eq, PartialEq, Copy, Clone)]
pub struct RawKey(TypeId);

impl RawKey {
    #[doc(hidden)]
    pub fn eq_other<T: ?Sized>(self, other: Key<T>) -> bool {
        self == other.id
    }
}

pub struct Key<T: ?Sized> {
    /// Parameter lifetime is invariant because users could potentially provide keys with an
    /// insufficient lifetime.
    _ty: PhantomData<std::cell::Cell<T>>,

    /// The unique identifier of the key.
    id: RawKey,
}

impl<T: ?Sized> Key<T> {
    #[doc(hidden)]
    pub const unsafe fn new<K: 'static>() -> Self {
        Self {
            _ty: PhantomData,
            id: RawKey (TypeId::of::<K>()),
        }
    }

    #[doc(hidden)]
    pub unsafe fn write_to_ptr(self, ptr: *mut (), field: &T) {
        ptr.cast::<&T>().write(field);
    }
}

impl<T: ?Sized> hash::Hash for Key<T> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state)
    }
}

impl<T: ?Sized> Eq for Key<T> {}
impl<T: ?Sized> PartialEq for Key<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id.eq(&other.id)
    }
}

impl<T: ?Sized> Copy for Key<T> {}
impl<T: ?Sized> Clone for Key<T> {
    fn clone(&self) -> Self {
        Self {
            _ty: PhantomData,
            id: self.id,
        }
    }
}

#[macro_export]
macro_rules! new_key {
    ($type:ty) => {
        {
            struct UniqueTy;

            unsafe {
                // Safety: `UniqueTy` is guaranteed to be a unique type.
                $crate::core::game_object::Key::<$type>::new::<UniqueTy>()
            }
        }
    };
}

// === Game Objects === //

pub unsafe trait GameObject {
    unsafe fn get_raw(&self, key: RawKey, out: *mut ()) -> bool;
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
        let mut field = MaybeUninit::<&'a T>::uninit();

        let has_field = unsafe {
            self.get_raw(key.id, field.as_mut_ptr().cast::<()>())
        };

        if has_field {
            Some(unsafe { field.assume_init() })
        } else {
            None
        }
    }
}

#[macro_export]
macro_rules! game_object {
    ($target:ty {
        $($key:path => $field:ident),*
        $(,)?
    }) => {
        unsafe impl $crate::core::game_object::GameObject for $target {
            unsafe fn get_raw(&self, key: $crate::core::game_object::RawKey, out: *mut ()) -> bool {
                $(
                    if key.eq_other($key) {  // $key is stable because it's a path.
                        $key.write_to_ptr(out, &self.$field);
                        true
                    } else
                )*

                { false }
            }
        }
    };
}
