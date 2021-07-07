use std::hash;
use std::intrinsics::type_id;
use std::marker::PhantomData;
use std::ops::Deref;
use crate::util::PhantomInvariant;

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
    /// Marker to bind the `T` generic parameter. Parameter lifetime is invariant because users could
    /// potentially provide keys with an insufficient lifetime.
    _ty: PhantomInvariant<T>,

    /// The program unique identifier of the key.
    raw_id: RawKey,
}

impl<T: ?Sized + 'static> Key<T> {
    /// Constructs the singleton key for the specified component type. As per [TypeId]'s
    /// limitations, this type must live for `'static`.
    pub const fn typed() -> Self {
        Self {
            _ty: PhantomInvariant::new(),
            raw_id: RawKey::new::<T>()
        }
    }
}

impl<T: ?Sized> Key<T> {
    /// Promotes a [RawKey] to a typed [Key]. This operation is super unsafe. To create new keys safely,
    /// use the [new_key] macro instead.
    ///
    /// ## Safety
    ///
    /// A given [RawId] must only ever be associated with a single type `T` and failing to do so will cause
    /// unsoundness in the type system.
    ///
    pub const unsafe fn new_unchecked(raw_id: RawKey) -> Self {
        Self {
            _ty: PhantomInvariant::new(),
            raw_id,
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
            _ty: PhantomInvariant::new(),
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
        Key::<$type>::new_unchecked(RawKey::new::<UniqueTy>())
    }
}

// === V-Tables === //

pub struct RawVTable {

}

pub struct VTableFrag<S, R: ?Sized> {
    _types: PhantomData<*const (S, R)>,
    raw: RawVTable,
}

pub macro vtable($target:ty) {

}

// === Obj Runtime === //

pub trait Comp where Self: Sized {
    /// The type of the root passed to this component through a [CompRef]. V-Tables must be formed
    /// of components with a homogeneous root type to be valid and [Obj] will only be implemented on
    /// a [Comp] that whose root type is applicable to `Self`.
    type Root: ?Sized + Obj = dyn Obj;

    /// A strongly-typed V-table that can be generated through the [vtable] macro.
    const FRAG: VTableFrag<Self, Self::Root>;
}

pub unsafe trait Obj {
    // TODO: Migrate to associated variable constants once available.
    fn table(&self) -> &'static RawVTable;
}

unsafe impl<T: ?Sized + Comp<Root = T>> Obj for T {
    fn table(&self) -> &'static RawVTable {
        &T::FRAG.raw
    }
}

pub struct CompRef<'a, T: ?Sized> {
    root: &'a dyn Obj,
    comp: &'a T,
}

impl<'a, T: ?Sized + Comp> CompRef<'a, T> {
    pub fn new(root: &'a T::Root, comp: &'a T) -> Self {
        Self { root: root as &'a dyn Obj, comp }
    }

    pub fn root(&self) -> &'a T::Root {
        let root = self.root as *const dyn Obj;  // Fetch the raw wide pointer
        let (root, _) = root.to_raw_parts();     // Fetch the address part
        let root = root.cast::<T::Root>();       // Cast it to the appropriate type
        unsafe { &*root }
    }
}

impl<'a, T: ?Sized> CompRef<'a, T> {
    pub unsafe fn new_unsafe(root: &'a dyn Obj, comp: &'a T) -> Self {
        Self { root, comp }
    }

    pub fn root_raw(&self) -> &'a dyn Obj {
        self.root
    }

    pub fn comp(&self) -> &'a T {
        self.comp
    }
}

impl<T: ?Sized + Comp> Deref for CompRef<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.comp
    }
}

impl<T: ?Sized> Copy for CompRef<'_, T> {}
impl<T: ?Sized> Clone for CompRef<'_, T> {
    fn clone(&self) -> Self {
        Self { root: self.root, comp: self.comp }
    }
}

pub trait ProviderExt {
    fn try_fetch_key<T: ?Sized + Comp<Root = Self>>(&self, key: Key<T>) -> Option<CompRef<T>>;

    fn fetch_key<T: ?Sized + Comp<Root = Self>>(&self, key: Key<T>) -> CompRef<T> {
        self.try_fetch_key(key).unwrap()
    }

    fn has_key<T: ?Sized + Comp<Root = Self>>(&self, key: Key<T>) -> bool {
        self.try_fetch_key(key).is_some()
    }

    fn try_fetch<T: ?Sized + 'static + Comp<Root = Self>>(&self) -> Option<CompRef<T>> {
        self.try_fetch_key(Key::<T>::typed())
    }

    fn fetch<T: ?Sized + 'static + Comp<Root = Self>>(&self) -> CompRef<T> {
        self.fetch_key(Key::<T>::typed())
    }
    fn has<T: ?Sized + 'static + Comp<Root = Self>>(&self) -> bool {
        self.has_key(Key::<T>::typed())
    }
}

impl<B: ?Sized + Obj> ProviderExt for B {
    fn try_fetch_key<T: ?Sized + Comp<Root = Self>>(&self, key: Key<T>) -> Option<CompRef<T>> {
        todo!()
    }
}

// === Utilities === //

// TODO: comp and deps generator macros
