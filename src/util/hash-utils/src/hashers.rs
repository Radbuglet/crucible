use std::{hash, marker::PhantomData};

use derive_where::derive_where;
use rustc_hash::FxHasher;

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

pub type FxBuildHasher = ConstBuildHasherDefault<FxHasher>;
pub type FxHashMap<K, V> = hashbrown::HashMap<K, V, FxBuildHasher>;
pub type FxHashSet<T> = hashbrown::HashSet<T, FxBuildHasher>;
pub type FxDashMap<K, V> = dashmap::DashMap<K, V, FxBuildHasher>;

pub type NopBuildHasher = ConstBuildHasherDefault<NopHasher>;
pub type NopHashMap<K, V> = hashbrown::HashMap<K, V, NopBuildHasher>;
pub type NopHashSet<T> = hashbrown::HashSet<T, NopBuildHasher>;
pub type NopDashMap<K, V> = dashmap::DashMap<K, V, NopBuildHasher>;
