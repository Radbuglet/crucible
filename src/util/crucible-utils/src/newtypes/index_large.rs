use std::{
    fmt, iter,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    vec,
};

use crucible_utils_proc::iterator;
use derive_where::derive_where;

use super::{Index, IndexSlice, IndexSliceIter, IndexSliceIterMut, IndexSliceKeys};

// === Traits === //

pub trait LargeIndex: Index {
    type Prim: num_traits::PrimInt;

    fn from_raw(idx: Self::Prim) -> Self;

    fn as_raw(self) -> Self::Prim;

    fn map_raw(self, f: impl FnOnce(Self::Prim) -> Self::Prim) -> Self {
        Self::from_raw(f(self.as_raw()))
    }

    fn update_raw(&mut self, f: impl FnOnce(Self::Prim) -> Self::Prim) {
        *self = self.map_raw(f)
    }
}

#[doc(hidden)]
pub mod define_index_internals {
    pub use {
        super::{
            super::{Index, IndexOptions},
            LargeIndex,
        },
        std::{convert::TryFrom, option::Option, primitive::usize},
    };

    pub const LARGE_INDEX_OPTIONS: IndexOptions = IndexOptions { use_map_fmt: false };
}

#[macro_export]
macro_rules! define_index {
    ($(
        $(#[$attr:meta])*
        $vis:vis struct $name:ident: $ty:ty;
    )*) => {$(
        $(#[$attr])*
        #[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
        $vis struct $name(pub $ty);

        impl $crate::newtypes::define_index_internals::Index for $name {
            const OPTIONS: $crate::newtypes::define_index_internals::IndexOptions =
                $crate::newtypes::define_index_internals::LARGE_INDEX_OPTIONS;

            fn try_from_usize(idx: $crate::newtypes::define_index_internals::usize) -> $crate::newtypes::define_index_internals::Option<Self> {
                <$ty as $crate::newtypes::define_index_internals::TryFrom<_>>::try_from(idx).ok().map(Self)
            }

            fn as_usize(self) -> $crate::newtypes::define_index_internals::usize {
                self.0 as $crate::newtypes::define_index_internals::usize
            }
        }

        impl $crate::newtypes::define_index_internals::LargeIndex for $name {
            type Prim = $ty;

            fn from_raw(raw: $ty) -> Self {
                Self(raw)
            }

            fn as_raw(self) -> $ty {
                self.0
            }
        }
    )*};
}

pub use define_index;

// === IndexVec === //

#[derive(Debug, Clone)]
#[iterator(V, &mut self.0)]
pub struct IndexVecIntoIter<V>(vec::IntoIter<V>);

#[derive(Debug, Clone)]
#[iterator((K, V), &mut self.0)]
pub struct IndexVecIntoEnumerate<K: LargeIndex, V>(
    iter::Zip<IndexSliceKeys<K>, IndexVecIntoIter<V>>,
);

#[derive_where(Clone; V)]
#[derive_where(Default)]
pub struct IndexVec<K, V> {
    pub _ty: PhantomData<fn(K) -> K>,
    pub raw: Vec<V>,
}

impl<K: Index, V: fmt::Debug> fmt::Debug for IndexVec<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

impl<K, V> IndexVec<K, V> {
    pub const fn new() -> Self {
        Self::from_raw(Vec::new())
    }

    pub const fn from_raw(raw: Vec<V>) -> Self {
        Self {
            _ty: PhantomData,
            raw,
        }
    }
}

impl<K: LargeIndex, V> IndexVec<K, V> {
    pub fn push(&mut self, value: V) -> K {
        let new_index = K::from_usize(self.raw.len());
        self.raw.push(value);
        new_index
    }

    pub fn pop(&mut self) -> Option<V> {
        self.raw.pop()
    }

    pub fn pop_entry(&mut self) -> Option<(K, V)> {
        self.raw.pop().map(|v| (self.len(), v))
    }

    pub fn entry_with(&mut self, key: K, f: impl FnMut() -> V) -> &mut V {
        if self.raw.len() < key.as_usize() + 1 {
            self.raw.resize_with(key.as_usize() + 1, f);
        }

        &mut self[key]
    }

    pub fn entry(&mut self, key: K) -> &mut V
    where
        V: Default,
    {
        self.entry_with(key, V::default)
    }

    pub fn into_enumerate(self) -> IndexVecIntoEnumerate<K, V> {
        IndexVecIntoEnumerate(self.keys().zip(self))
    }
}

impl<K, V> Deref for IndexVec<K, V> {
    type Target = IndexSlice<K, V>;

    fn deref(&self) -> &Self::Target {
        IndexSlice::from_raw_ref(&self.raw)
    }
}

impl<K, V> DerefMut for IndexVec<K, V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        IndexSlice::from_raw_mut(&mut self.raw)
    }
}

impl<'a, K: LargeIndex, V> IntoIterator for &'a IndexVec<K, V> {
    type IntoIter = IndexSliceIter<'a, V>;
    type Item = &'a V;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, K: LargeIndex, V> IntoIterator for &'a mut IndexVec<K, V> {
    type IntoIter = IndexSliceIterMut<'a, V>;
    type Item = &'a mut V;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<K, V> IntoIterator for IndexVec<K, V> {
    type Item = V;
    type IntoIter = IndexVecIntoIter<V>;

    fn into_iter(self) -> Self::IntoIter {
        IndexVecIntoIter(self.raw.into_iter())
    }
}

impl<K, V> FromIterator<V> for IndexVec<K, V> {
    fn from_iter<T: IntoIterator<Item = V>>(iter: T) -> Self {
        Self::from_raw(Vec::from_iter(iter))
    }
}
