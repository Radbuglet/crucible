#![allow(unused_variables)]

use std::collections::HashMap;
use std::collections::hash_map::RandomState;
use std::hash::{Hash, BuildHasher};
use std::marker::PhantomData;
use std::borrow::Borrow;

pub trait Weak {
    fn is_alive(&self) -> bool;
}

pub trait WeakMapDef {
    type Key;
    type Value;

    fn is_alive(key: &Self::Key, value: &Self::Value) -> bool;
}

pub struct WeakMap<D: WeakMapDef, S = RandomState> {
    _map: HashMap<D::Key, D::Value, S>,
    def: PhantomData<D>,
}

impl<D: WeakMapDef> WeakMap<D> {
    pub fn new() -> Self {
        todo!()
    }

    pub fn with_capacity(capacity: usize) -> Self {
        todo!()
    }
}

impl<D: WeakMapDef, S> WeakMap<D, S> {
    pub fn with_hasher(hash_builder: S) -> Self {
        todo!()
    }

    pub fn with_capacity_and_hasher(capacity: usize, hash_builder: S) -> Self {
        todo!()
    }

    pub fn capacity(&self) -> usize {
        todo!()
    }

    pub fn hasher(&self) -> &S {
        todo!()
    }

    pub fn keys(&self) -> () {
        todo!()
    }

    pub fn values(&self) -> () {
        todo!()
    }

    pub fn values_mut(&self) -> () {
        todo!()
    }

    pub fn iter(&self) -> () {
        todo!()
    }

    pub fn iter_mut(&self) -> () {
        todo!()
    }

    pub fn clear(&mut self) {
        todo!()
    }
}

impl<D: WeakMapDef, S> WeakMap<D, S>
where
    D::Key: Hash + Eq,
    S: BuildHasher
{
    pub fn reserve(&mut self, additional: usize) {
        todo!()
    }

    pub fn shrink_to_fit(&mut self) {
        todo!()
    }

    pub fn shrink_to(&mut self, min_capacity: usize) {
        todo!()
    }

    pub fn insert(&mut self, k: D::Key, v: D::Value) -> Option<D::Value> {
        todo!()
    }

    pub fn remove<Q: ?Sized>(&self, k: &Q) -> Option<D::Value>
    where
        D::Key: Borrow<Q>,
        Q: Hash + Eq,
    {
        todo!()
    }

    pub fn get<Q: ?Sized>(&self, k: &Q) -> Option<&D::Value>
    where
        D::Key: Borrow<Q>,
        Q: Hash + Eq,
    {
        todo!()
    }

    pub fn get_mut<Q: ?Sized>(&self, k: &Q) -> Option<&D::Value>
    where
        D::Key: Borrow<Q>,
        Q: Hash + Eq,
    {
        todo!()
    }

    pub fn get_key_value<Q: ?Sized>(&self, k: &Q) -> Option<(&D::Key, &D::Value)>
    where
        D::Key: Borrow<Q>,
        Q: Hash + Eq,
    {
        todo!()
    }

    pub fn contains_key<Q: ?Sized>(&self, k: &Q) -> bool
    where
        D::Key: Borrow<Q>,
        Q: Hash + Eq,
    {
        todo!()
    }

    pub fn retain<F>(&mut self, f: F)
    where
        F: FnMut(&D::Key, &mut D::Value) -> bool,
    {
        todo!()
    }
}

// TODO: Impl: Eq, Debug, Default, Index

pub type WeakMapK<K, V, S = RandomState> = WeakMap<WeakMapDefK<K, V>, S>;
pub type WeakMapV<K, V, S = RandomState> = WeakMap<WeakMapDefV<K, V>, S>;
pub type WeakMapKV<K, V, S = RandomState> = WeakMap<WeakMapDefKV<K, V>, S>;

pub struct WeakMapDefK<K, V>(PhantomData<*const (K, V)>);

impl<K: Weak, V> WeakMapDef for WeakMapDefK<K, V> {
    type Key = K;
    type Value = V;

    fn is_alive(key: &Self::Key, _: &Self::Value) -> bool {
        key.is_alive()
    }
}

pub struct WeakMapDefV<K, V>(PhantomData<*const (K, V)>);

impl<K, V: Weak> WeakMapDef for WeakMapDefV<K, V> {
    type Key = K;
    type Value = V;

    fn is_alive(_: &Self::Key, value: &Self::Value) -> bool {
        value.is_alive()
    }
}

pub struct WeakMapDefKV<K, V>(PhantomData<*const (K, V)>);

impl<K: Weak, V: Weak> WeakMapDef for WeakMapDefKV<K, V> {
    type Key = K;
    type Value = V;

    fn is_alive(key: &Self::Key, value: &Self::Value) -> bool {
        key.is_alive() && value.is_alive()
    }
}
