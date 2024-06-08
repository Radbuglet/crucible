use std::{
    fmt, hash, iter,
    marker::PhantomData,
    ops::{self, Deref, DerefMut},
    slice,
};

use derive_where::derive_where;
use newtypes_proc::{iterator, transparent};

// === Index === //

pub trait Index: fmt::Debug + Copy + hash::Hash + Eq + Ord {
    type Prim: num_traits::PrimInt;

    fn try_from_usize(idx: usize) -> Option<Self>;

    fn from_usize(idx: usize) -> Self {
        Self::try_from_usize(idx).unwrap()
    }

    fn as_usize(self) -> usize;

    fn from_raw(idx: Self::Prim) -> Self;

    fn as_raw(self) -> Self::Prim;

    fn map_raw(self, f: impl FnOnce(Self::Prim) -> Self::Prim) -> Self {
        Self::from_raw(f(self.as_raw()))
    }

    fn map_usize(self, f: impl FnOnce(usize) -> usize) -> Self {
        Self::from_usize(f(self.as_usize()))
    }

    fn update_raw(&mut self, f: impl FnOnce(Self::Prim) -> Self::Prim) {
        *self = self.map_raw(f)
    }

    fn update_usize(&mut self, f: impl FnOnce(usize) -> usize) {
        *self = self.map_usize(f)
    }
}

#[doc(hidden)]
pub mod define_index_internals {
    pub use {
        super::Index,
        std::{convert::TryFrom, option::Option, primitive::usize},
    };
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

        impl $crate::define_index_internals::Index for $name {
            type Prim = $ty;

            fn try_from_usize(idx: $crate::define_index_internals::usize) -> $crate::define_index_internals::Option<Self> {
                <$ty as $crate::define_index_internals::TryFrom<_>>::try_from(idx).ok().map(Self)
            }

            fn from_raw(raw: $ty) -> Self {
                Self(raw)
            }

            fn as_usize(self) -> $crate::define_index_internals::usize {
                self.0 as $crate::define_index_internals::usize
            }

            fn as_raw(self) -> $ty {
                self.0
            }
        }
    )*};
}

// === IndexSlice === //

#[derive(Debug)]
#[derive_where(Clone)]
#[iterator(&'a V, &mut self.0)]
pub struct IndexSliceIter<'a, V>(slice::Iter<'a, V>);

#[derive(Debug)]
#[iterator(&'a mut V, &mut self.0)]
pub struct IndexSliceIterMut<'a, V>(slice::IterMut<'a, V>);

#[derive(Debug)]
#[derive_where(Clone)]
#[iterator((K, &'a V), &mut self.0)]
pub struct IndexSliceIterEnumerate<'a, K: Index, V>(
    iter::Zip<IndexSliceKeys<K>, IndexSliceIter<'a, V>>,
);

#[derive(Debug)]
#[iterator((K, &'a mut V), &mut self.0)]
pub struct IndexSliceIterEnumerateMut<'a, K: Index, V>(
    iter::Zip<IndexSliceKeys<K>, IndexSliceIterMut<'a, V>>,
);

#[derive_where(Debug, Clone)]
pub struct IndexSliceKeys<K: Index> {
    curr: K,
    end_excl: K,
}

impl<K: Index> Iterator for IndexSliceKeys<K> {
    type Item = K;

    fn next(&mut self) -> Option<Self::Item> {
        (self.curr < self.end_excl).then(|| {
            let curr = self.curr;
            self.curr.update_usize(|v| v + 1);
            curr
        })
    }
}

#[transparent(raw, pub from_raw)]
#[repr(transparent)]
pub struct IndexSlice<K, V> {
    pub _ty: PhantomData<fn(K) -> K>,
    pub raw: [V],
}

impl<K, V: fmt::Debug> fmt::Debug for IndexSlice<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(&self.raw).finish()
    }
}

impl<K: Index, V> ops::Index<K> for IndexSlice<K, V> {
    type Output = V;

    fn index(&self, index: K) -> &Self::Output {
        &self.raw[index.as_usize()]
    }
}

impl<K: Index, V> ops::IndexMut<K> for IndexSlice<K, V> {
    fn index_mut(&mut self, index: K) -> &mut Self::Output {
        &mut self.raw[index.as_usize()]
    }
}

impl<K: Index, V> IndexSlice<K, V> {
    pub fn get(&self, idx: K) -> Option<&V> {
        self.raw.get(idx.as_usize())
    }

    pub fn get_mut(&mut self, idx: K) -> Option<&mut V> {
        self.raw.get_mut(idx.as_usize())
    }

    pub fn len(&self) -> K {
        K::from_usize(self.raw.len())
    }

    pub fn is_empty(&self) -> bool {
        self.raw.is_empty()
    }

    pub fn keys(&self) -> IndexSliceKeys<K> {
        IndexSliceKeys {
            curr: K::from_usize(0),
            end_excl: self.len(),
        }
    }

    pub fn iter(&self) -> IndexSliceIter<'_, V> {
        IndexSliceIter(self.raw.iter())
    }

    pub fn iter_mut(&mut self) -> IndexSliceIterMut<'_, V> {
        IndexSliceIterMut(self.raw.iter_mut())
    }

    pub fn enumerate(&self) -> IndexSliceIterEnumerate<'_, K, V> {
        IndexSliceIterEnumerate(self.keys().zip(self.iter()))
    }

    pub fn enumerate_mut(&mut self) -> IndexSliceIterEnumerateMut<'_, K, V> {
        IndexSliceIterEnumerateMut(self.keys().zip(self.iter_mut()))
    }
}

// === IndexVec === //

#[derive_where(Clone; V)]
#[derive_where(Default)]
pub struct IndexVec<K, V> {
    pub _ty: PhantomData<fn(K) -> K>,
    pub raw: Vec<V>,
}

impl<K, V: fmt::Debug> fmt::Debug for IndexVec<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
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

impl<K, V> FromIterator<V> for IndexVec<K, V> {
    fn from_iter<T: IntoIterator<Item = V>>(iter: T) -> Self {
        Self::from_raw(Vec::from_iter(iter))
    }
}

impl<K: Index, V> IndexVec<K, V> {
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
}
