use std::hash;
use std::intrinsics::type_id;
use std::marker::PhantomData;
use std::num::NonZeroU64;
use crate::util::PhantomInvariant;

/// An untyped component identifier underlying [Key].
// We assume that type IDs are non-zero because getting that exact ID is exceedingly unlikely and
// doing so enables specific memory and performance optimizations in `PerfectMap`.
#[derive(Hash, Eq, PartialEq, Copy, Clone)]
pub struct RawKey(NonZeroU64);

impl RawKey {
    pub const fn new<T: ?Sized + 'static>() -> Self {
        if let Some(id) = NonZeroU64::new(type_id::<T>()) {
            Self (id)
        } else {
            panic!("RawKey had a TypeId of `0`, breaking `PerfectMap`'s invariants. This is \
                    exceedingly rare and a recompile should fix this once in a lifetime occurrence.");
        }
    }

    #[inline(always)]
    pub(crate) const fn as_u64(self) -> NonZeroU64 {
        self.0
    }

    pub(crate) const fn const_eq(self, other: Self) -> bool {
        self.0.get() == other.0.get()
    }
}

/// A unique identifier for a component of type `T`.
pub struct Key<T: ?Sized> {
    /// Marker to bind the `T` generic parameter. Parameter lifetime is invariant because users could
    /// potentially provide keys with an insufficient lifetime.
    ty: PhantomInvariant<T>,

    /// The program unique identifier of the key.
    raw_id: RawKey,
}

impl<T: ?Sized + 'static> Key<T> {
    /// Constructs the singleton key for the specified component type. As per [TypeId]'s
    /// limitations, this type must live for `'static`.
    pub const fn typed() -> Self {
        Self {
            ty: PhantomData,
            raw_id: RawKey::new::<T>(),
        }
    }
}

impl<T: ?Sized> Key<T> {
    /// Promotes a [RawKey] to a typed [Key]. This operation is super unsafe. To create new keys safely,
    /// use the [new_key] macro instead.
    ///
    /// ## Safety
    ///
    /// A given [RawId] must only ever be associated with a single type `T` and failing to do so will
    /// cause unsoundness in the type system.
    ///
    pub const unsafe fn new_unchecked(raw_id: RawKey) -> Self {
        Self {
            ty: PhantomData,
            raw_id,
        }
    }

    pub const fn raw(self) -> RawKey {
        self.raw_id
    }
}

impl<T: ?Sized> Into<RawKey> for Key<T> {
    fn into(self) -> RawKey {
        self.raw()
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
            ty: PhantomData,
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
pub macro new_key($type:ty) {
    unsafe {
        struct UniqueTy;

        // Safety: `UniqueTy` is guaranteed to be a unique type.
        Key::<$type>::new_unchecked(RawKey::new::<UniqueTy>())
    }
}
