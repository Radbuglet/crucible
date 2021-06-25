// TODO: Finish docs, tests

use std::any::TypeId;
use std::hash;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ops::Deref;
use std::ptr::NonNull;

// === Keys === //

/// An untyped program unique identifier underlying [Key].
#[derive(Hash, Eq, PartialEq, Copy, Clone)]
struct RawKey(TypeId);

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

/// An object-safe trait to fetch a requested component. While it is perfectly fine to implement this
/// trait manually, the [provide] macro is much less verbose and generally provides sufficient flexibility.
///
/// ## Examples
///
/// TODO
///
pub trait Provider {
    /// An object-safe method to fetch the requested component. Implementors provide components using
    /// the [KeyOut::provide] and [KeyOut::provide_key] methods or by calling a sub object's
    /// [Provider::get_raw] method.
    fn get_raw<'val>(&'val self, out: &mut KeyOut<'_, 'val>);
}

pub macro provide(  // TODO: Generics, getter methods, and docs
    $target:ty
    $([$(.$field:ident),*])?
    $(=> $($type:ty),*)?
) {
    impl Provider for $target {
        #[allow(unused)]  // For empty implementations
        fn get_raw<'val>(&'val self, out: &mut KeyOut<'_, 'val>) {
            // Forces method resolution on `Provider` trait.
            fn get_raw<'a, T: ?Sized + Provider>(out: &mut KeyOut<'_, 'a>, field: &'a T) {
                field.get_raw(out);
            }

            $($(
                get_raw(out, &self.$field);
            )*)?

            $($(
                out.provide::<$type>(self);
            )*)?
        }
    }
}

/// The "out parameter" for [Provider]. Users request
pub struct KeyOut<'view, 'val> {
    ptr_ty: PhantomData<&'view mut &'val ()>,
    ptr: NonNull<()>,  // NonNull for niche representation
    raw_key: RawKey,
    is_init: bool,
}

impl<'view, 'val> KeyOut<'view, 'val> {
    fn new<T: ?Sized>(key: Key<T>, target: &'view mut MaybeUninit<&'val T>) -> Self {
        Self {
            ptr_ty: PhantomData,
            ptr: NonNull::from(target).cast::<()>(),
            raw_key: key.raw_id,
            is_init: false,
        }
    }

    /// Returns whether or not this [KeyOut] has been initialized with with a valid component. Can be
    /// used to short-circuit a search after dynamically dispatching a call to [Provider::get_raw].
    /// In cases where the compiler has full knowledge of the [Provider::get_raw] implementation (e.g.
    /// static dispatch without the use of [std::hint::blackbox]), the compiler should be smart enough
    /// to detect mutual exclusion between multiple [KeyOut::provide] calls.
    pub fn is_init(&self) -> bool {
        self.is_init
    }

    /// Fills the [KeyOut] with the specified component if the user-requested key matches the provided
    /// `field_key`. Replaces any existing component values.
    pub fn provide_key<T: ?Sized>(&mut self, field_key: Key<T>, field_ref: &'val T) {
        if field_key.raw_id == self.raw_key {
            unsafe {
                self.ptr.as_ptr().cast::<&'val T>().write(field_ref);
            }
            self.is_init = true;
        }
    }

    /// Fills the [KeyOut] with the specified component if the user-requested key matches the type's
    /// [singleton key](Key::typed).
    pub fn provide<T: ?Sized + 'static>(&mut self, field_ref: &'val T) {
        self.provide_key(Key::typed(), field_ref)
    }
}

// === Extension methods === //

/// A wrapper around a component instance that bundles it with the `Provider` from which it was fetched.
///
/// TODO: Clear up semantics for `ObjAncestry`.
pub struct Comp<'a, O: ?Sized, T: ?Sized> {
    obj: &'a O,
    comp: &'a T,
}

// TODO: Coercions

impl<'a, O: ?Sized, T: ?Sized> Comp<'a, O, T> {
    pub fn new(obj: &'a O, comp: &'a T) -> Self {
        Self { obj, comp }
    }

    pub fn obj_raw(&self) -> &'a O {
        self.obj
    }

    pub fn comp_raw(&self) -> &'a T {
        self.comp
    }
}

impl<O: ?Sized, T: ?Sized> Deref for Comp<'_, O, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.comp
    }
}

pub trait ProviderExt {
    type Obj: ?Sized;

    fn try_fetch_key<T: ?Sized>(&self, key: Key<T>) -> Option<Comp<Self::Obj, T>>;

    fn fetch_key<T: ?Sized>(&self, key: Key<T>) -> Comp<Self::Obj, T> {
        self.try_fetch_key(key).unwrap()
    }

    fn has_key<T: ?Sized>(&self, key: Key<T>) -> bool {
        self.try_fetch_key(key).is_some()
    }

    fn try_fetch<T: ?Sized + 'static>(&self) -> Option<Comp<Self::Obj, T>> {
        self.try_fetch_key(Key::<T>::typed())
    }

    fn fetch<T: ?Sized + 'static>(&self) -> Comp<Self::Obj, T> {
        self.fetch_key(Key::<T>::typed())
    }

    fn has<T: ?Sized + 'static>(&self) -> bool {
        self.has_key(Key::<T>::typed())
    }

    fn try_fetch_many<'a, T: FetchMany<&'a Self>>(&'a self) -> Option<T> {
        T::inner_fetch_many(self)
    }

    fn fetch_many<'a, T: FetchMany<&'a Self>>(&'a self) -> T {
        T::inner_fetch_many(self).unwrap()
    }
}

impl<B: ?Sized + Provider> ProviderExt for B {
    type Obj = Self;

    fn try_fetch_key<'a, T: ?Sized>(&'a self, key: Key<T>) -> Option<Comp<'a, Self, T>> {
        let mut target = MaybeUninit::<&'a T>::uninit();
        let mut view = KeyOut::new(key, &mut target);

        self.get_raw(&mut view);

        if view.is_init() {
            Some (Comp::new(self, unsafe { target.assume_init() }))
        } else {
            None
        }
    }
}

// === Tuple stuff === //

pub trait FetchMany<Obj>: Sized {
    fn inner_fetch_many(obj: Obj) -> Option<Self>;
}

// Constructs an expression guaranteed to return a tuple, regardless of the number of elements provided.
macro tup {
    // A special case to construct an empty tuple (`(,)` is illegal).
    () => { () },

    // For tuples with more than one element
    ($($elem:expr),+) => {
        (
            $($ elem),+
            ,  // A trailing comma forces the parser to treat the parens as a tuple and not an expression.
        )
    }
}

macro impl_tuple($($name:ident : $idx:tt),*) {
    // Fetching for `(&A, ..., &Z)`
    impl<'a, Obj: ?Sized + Provider, $($name: ?Sized + 'static),*> FetchMany<&'a Obj> for ($(Comp<'a, Obj, $name>,)*) {
        #[allow(unused)]  // For empty tuples
        fn inner_fetch_many(obj: &'a Obj) -> Option<Self> {
            Some (tup!(
                $(obj.try_fetch::<$name>()?),*
            ))
        }
    }

    // Providing for `(A, ..., Z)`
    impl<'me, $($name: Provider + 'static),*> Provider for ($($name,)*) {
        #[allow(unused)]  // For empty tuples
        fn get_raw<'val>(&'val self, out: &mut KeyOut<'_, 'val>) {
            $(self.$idx.get_raw(out);)*
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
