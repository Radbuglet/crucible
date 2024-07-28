use std::{fmt, hash, iter, marker::PhantomData, ops, slice};

use derive_where::derive_where;

use super::{iterator, transparent};

// === Index === //

#[derive(Debug, Copy, Clone, Default)]
pub struct IndexOptions {
    pub use_map_fmt: bool,
}

pub trait Index: 'static + Sized + fmt::Debug + Copy + hash::Hash + Eq + Ord {
    const OPTIONS: IndexOptions;

    fn try_from_usize(idx: usize) -> Option<Self>;

    fn from_usize(idx: usize) -> Self {
        Self::try_from_usize(idx).unwrap()
    }

    fn as_usize(self) -> usize;

    fn map_usize(self, f: impl FnOnce(usize) -> usize) -> Self {
        Self::from_usize(f(self.as_usize()))
    }

    fn update_usize(&mut self, f: impl FnOnce(usize) -> usize) {
        *self = self.map_usize(f)
    }
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

impl<K: Index, V: fmt::Debug> fmt::Debug for IndexSlice<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if K::OPTIONS.use_map_fmt {
            f.debug_map().entries(self.enumerate()).finish()
        } else {
            f.debug_list().entries(&self.raw).finish()
        }
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

impl<'a, K: Index, V> IntoIterator for &'a IndexSlice<K, V> {
    type Item = &'a V;
    type IntoIter = IndexSliceIter<'a, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, K: Index, V> IntoIterator for &'a mut IndexSlice<K, V> {
    type Item = &'a mut V;
    type IntoIter = IndexSliceIterMut<'a, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}
