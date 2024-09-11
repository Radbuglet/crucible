use std::{
    hash::{self, BuildHasher, Hasher},
    marker::PhantomData,
};

use derive_where::derive_where;

pub use rustc_hash::FxHasher;

use super::{SliceMap, StrMap};

// == Aliases === //

pub type FxBuildHasher = ConstBuildHasherDefault<FxHasher>;
pub type FxHashMap<K, V> = hashbrown::HashMap<K, V, FxBuildHasher>;
pub type FxHashSet<T> = hashbrown::HashSet<T, FxBuildHasher>;

pub type NopBuildHasher = ConstBuildHasherDefault<NopHasher>;
pub type NopHashMap<K, V> = hashbrown::HashMap<K, V, NopBuildHasher>;
pub type NopHashSet<T> = hashbrown::HashSet<T, NopBuildHasher>;

pub type FxSliceMap<K, V> = SliceMap<K, V, FxBuildHasher>;
pub type FxStrMap<V> = StrMap<V, FxBuildHasher>;

// === Hashers === //

#[derive_where(Debug, Copy, Clone, Default)]
pub struct ConstBuildHasherDefault<T> {
    _ty: PhantomData<fn() -> T>,
}

impl<T> ConstBuildHasherDefault<T> {
    pub const fn new() -> Self {
        Self { _ty: PhantomData }
    }
}

impl<T: Default + hash::Hasher> hash::BuildHasher for ConstBuildHasherDefault<T> {
    type Hasher = T;

    fn build_hasher(&self) -> Self::Hasher {
        T::default()
    }
}

#[derive(Debug, Default)]
pub struct NopHasher(u64);

impl hash::Hasher for NopHasher {
    fn write(&mut self, _bytes: &[u8]) {
        unimplemented!("`NopHasher` only supports `write_u64`");
    }

    fn write_u64(&mut self, v: u64) {
        self.0 = v;
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

// === Hash functions === //

pub fn fx_hash_one(value: impl hash::Hash) -> u64 {
    FxBuildHasher::new().hash_one(value)
}

// === Hash Adapters === //

pub trait BuildHasherExt: BuildHasher {
    fn hash_iter<T: hash::Hash>(&self, iter: impl IntoIterator<Item = T>) -> u64 {
        let mut hasher = self.build_hasher();
        let mut iter_len = 0;
        for elem in iter {
            elem.hash(&mut hasher);
            iter_len += 1;
        }
        hasher.write_usize(iter_len);
        hasher.finish()
    }
}

impl<H: ?Sized + BuildHasher> BuildHasherExt for H {}
