use std::{fmt, hash, iter, marker::PhantomData, mem, ops, slice, usize};

use derive_where::derive_where;
use num_traits::PrimInt;

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
#[iterator(&'a V, self.0.next())]
pub struct IndexSliceIter<'a, V>(slice::Iter<'a, V>);

#[derive(Debug)]
#[iterator(&'a mut V, self.0.next())]
pub struct IndexSliceIterMut<'a, V>(slice::IterMut<'a, V>);

#[derive(Debug)]
#[derive_where(Clone)]
#[iterator((K, &'a V), self.0.next())]
pub struct IndexSliceIterEnumerate<'a, K: Index, V>(
    iter::Zip<IndexSliceKeys<K>, IndexSliceIter<'a, V>>,
);

#[derive(Debug)]
#[iterator((K, &'a mut V), self.0.next())]
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

// === IndexBitSlice === //

#[iterator(K, self.0.next())]
pub struct IndexBitSliceIterOne<'a, K: Index, V: PrimInt>(
    IndexBitSliceIter<K, iter::Copied<slice::Iter<'a, V>>>,
);

#[iterator(K, self.0.next())]
pub struct IndexBitSliceIterZero<'a, K: Index, V: PrimInt>(
    IndexBitSliceIter<K, InvertBits<iter::Copied<slice::Iter<'a, V>>>>,
);

#[iterator(I::Item, self.0.next().map(|v| !v))]
struct InvertBits<I>(I)
where
    I: Iterator,
    I::Item: PrimInt;

pub struct IndexBitSliceIter<K, W>
where
    K: Index,
    W: Iterator,
    W::Item: PrimInt,
{
    _ty: PhantomData<fn(K) -> K>,
    words: W,
    word_idx_base: usize,
    word: W::Item,
}

impl<K, V, W> IndexBitSliceIter<K, W>
where
    K: Index,
    W: Iterator<Item = V>,
    V: PrimInt,
{
    pub fn new(words: W) -> Self {
        Self {
            _ty: PhantomData,
            words,
            word_idx_base: usize::MAX - IndexBitSlice::<K, V>::BITS_PER_WORD + 1,
            word: V::zero(),
        }
    }
}

impl<K, V, W> Iterator for IndexBitSliceIter<K, W>
where
    K: Index,
    W: Iterator<Item = V>,
    V: PrimInt,
{
    type Item = K;

    fn next(&mut self) -> Option<Self::Item> {
        while self.word.is_zero() {
            self.word = self.words.next()?;
            self.word_idx_base = self
                .word_idx_base
                .wrapping_add(IndexBitSlice::<K, V>::BITS_PER_WORD);
        }

        let idx = self.word.trailing_zeros() as usize;
        self.word = self.word & !(V::one() << idx);

        K::try_from_usize(self.word_idx_base + idx)
    }
}

#[transparent(raw, pub from_raw)]
#[repr(transparent)]
pub struct IndexBitSlice<K, V> {
    pub _ty: PhantomData<fn(K) -> K>,
    pub raw: [V],
}

impl<K: Index, V: PrimInt> fmt::Debug for IndexBitSlice<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.iter_ones()).finish()
    }
}

impl<K: Index, V: PrimInt> IndexBitSlice<K, V> {
    pub const BITS_PER_WORD: usize = mem::size_of::<V>() * 8;

    fn word_index(idx: usize) -> usize {
        idx / Self::BITS_PER_WORD
    }

    fn bit_index(idx: usize) -> usize {
        idx % Self::BITS_PER_WORD
    }

    fn bit_mask(idx: usize) -> V {
        V::one() << Self::bit_index(idx)
    }

    pub fn get(&self, idx: K) -> bool {
        let idx = idx.as_usize();
        !(self.raw[Self::word_index(idx)] & Self::bit_mask(idx)).is_zero()
    }

    pub fn set(&mut self, idx: K, value: bool) {
        let idx = idx.as_usize();
        let word = &mut self.raw[Self::word_index(idx)];
        if value {
            *word = *word | Self::bit_mask(idx);
        } else {
            *word = *word & !Self::bit_mask(idx);
        }
    }

    pub fn add(&mut self, idx: K) {
        self.set(idx, true);
    }

    pub fn remove(&mut self, idx: K) {
        self.set(idx, false);
    }

    pub fn add_all(&mut self) {
        for v in &mut self.raw {
            *v = V::max_value();
        }
    }

    pub fn remove_all(&mut self) {
        for v in &mut self.raw {
            *v = V::zero();
        }
    }

    pub fn iter_ones(&self) -> IndexBitSliceIterOne<'_, K, V> {
        IndexBitSliceIterOne(IndexBitSliceIter::new(self.raw.iter().copied()))
    }

    pub fn iter_zeros(&self) -> IndexBitSliceIterZero<'_, K, V> {
        IndexBitSliceIterZero(IndexBitSliceIter::new(InvertBits(self.raw.iter().copied())))
    }
}
