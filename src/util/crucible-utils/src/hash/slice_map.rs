use core::{hash, str};
use std::{fmt, marker::PhantomData, mem, ops::Range};

use crucible_utils_proc::iterator;
use derive_where::derive_where;
use hashbrown::{hash_map, HashMap};

use super::BuildHasherExt as _;
use crate::mem::{defuse, guard, smuggle_drop};

// === SliceMap === //

#[derive_where(Default; S)]
#[derive(Clone)]
pub struct SliceMap<K, V, S> {
    buf: Vec<K>,
    map: HashMap<Key, V, S>,
}

impl<K, V, S> fmt::Debug for SliceMap<K, V, S>
where
    K: fmt::Debug,
    V: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

impl<K, V, S> SliceMap<K, V, S> {
    pub fn new() -> Self
    where
        S: Default,
    {
        Self::default()
    }

    pub fn with_hasher(hash_builder: S) -> Self {
        Self {
            buf: Vec::new(),
            map: HashMap::with_hasher(hash_builder),
        }
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn iter(&self) -> SliceMapIter<'_, K, V> {
        SliceMapIter {
            buf: &self.buf,
            iter: self.map.iter(),
        }
    }

    pub fn iter_mut(&mut self) -> SliceMapIterMut<'_, K, V> {
        SliceMapIterMut {
            buf: &self.buf,
            iter: self.map.iter_mut(),
        }
    }

    pub fn keys(&self) -> SliceMapKeys<'_, K, V> {
        SliceMapKeys {
            buf: &self.buf,
            iter: self.map.keys(),
        }
    }

    pub fn values(&self) -> SliceMapValues<'_, V> {
        SliceMapValues {
            iter: self.map.values(),
        }
    }

    pub fn values_mut(&mut self) -> SliceMapValuesMut<'_, V> {
        SliceMapValuesMut {
            iter: self.map.values_mut(),
        }
    }

    pub fn key_buf(&self) -> &[K] {
        &self.buf
    }
}

impl<K, V, S> SliceMap<K, V, S>
where
    K: hash::Hash + Eq,
    S: hash::BuildHasher,
{
    pub fn entry(&mut self, key: impl IntoIterator<Item = K>) -> SliceMapEntry<'_, K, V, S> {
        // Stash the key away in `buf`.
        let old_buf_len = self.buf.len();
        let mut buf = guard(&mut self.buf, |buf| {
            buf.truncate(old_buf_len);
        });
        buf.extend(key);
        let key = &buf[old_buf_len..];

        // Find the entry in the map.
        let hash = self.map.hasher().hash_iter(key);
        let entry = self
            .map
            .raw_entry_mut()
            .from_hash(hash, |k| k.hash == hash && &buf[k.range()] == key);

        // Wrap the entry.
        match entry {
            hash_map::RawEntryMut::Occupied(entry) => {
                drop(buf); // (truncate the buffer)
                SliceMapEntry::Occupied(SliceMapEntryOccupied {
                    _ty: PhantomData,
                    key_buf: &self.buf,
                    key_range: entry.key().range(),
                    value: entry.into_mut(),
                })
            }
            hash_map::RawEntryMut::Vacant(entry) => {
                defuse(buf); // (transfer authority over truncation to `SliceMapEntryVacant`'s destructor)
                SliceMapEntry::Vacant(SliceMapEntryVacant(SliceMapEntryVacantInner {
                    key_buf: &mut self.buf,
                    retain_len: old_buf_len,
                    hash,
                    entry,
                }))
            }
        }
    }

    pub fn insert(&mut self, key: impl IntoIterator<Item = K>, value: V) -> Option<V> {
        match self.entry(key) {
            SliceMapEntry::Occupied(mut entry) => Some(entry.insert(value)),
            SliceMapEntry::Vacant(entry) => {
                entry.insert(value);
                None
            }
        }
    }

    pub fn get<'v, KI>(&self, key: KI) -> Option<&V>
    where
        KI: IntoIterator<Item = &'v K> + Clone,
        K: 'v,
    {
        let hash = self.map.hasher().hash_iter(key.clone());

        self.map
            .raw_entry()
            .from_hash(hash, |k| self.buf[k.range()].iter().eq(key.clone()))
            .map(|(_k, v)| v)
    }

    pub fn get_mut<'v, KI>(&mut self, key: KI) -> Option<&mut V>
    where
        KI: IntoIterator<Item = &'v K> + Clone,
        K: 'v,
    {
        let hash = self.map.hasher().hash_iter(key.clone());

        match self
            .map
            .raw_entry_mut()
            .from_hash(hash, |k| self.buf[k.range()].iter().eq(key.clone()))
        {
            hash_map::RawEntryMut::Occupied(entry) => Some(entry.into_mut()),
            hash_map::RawEntryMut::Vacant(_) => None,
        }
    }

    pub fn contains<'v, KI>(&self, key: KI) -> bool
    where
        KI: IntoIterator<Item = &'v K> + Clone,
        K: 'v,
    {
        self.get(key).is_some()
    }
}

impl<'a, K, V, S> IntoIterator for &'a SliceMap<K, V, S> {
    type Item = (&'a [K], &'a V);
    type IntoIter = SliceMapIter<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, K, V, S> IntoIterator for &'a mut SliceMap<K, V, S> {
    type Item = (&'a [K], &'a mut V);
    type IntoIter = SliceMapIterMut<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

#[derive(Copy, Clone)]
struct Key {
    hash: u64,
    from: usize,
    to: usize,
}

impl Key {
    fn range(self) -> Range<usize> {
        self.from..self.to
    }
}

// === SliceMapEntryMut === //

pub enum SliceMapEntry<'a, K, V, S> {
    Occupied(SliceMapEntryOccupied<'a, K, V, S>),
    Vacant(SliceMapEntryVacant<'a, K, V, S>),
}

impl<'a, K, V, S> SliceMapEntry<'a, K, V, S> {
    pub fn insert(self, value: V) -> SliceMapEntryOccupied<'a, K, V, S> {
        match self {
            SliceMapEntry::Occupied(mut entry) => {
                entry.insert(value);
                entry
            }
            SliceMapEntry::Vacant(entry) => entry.insert_entry(value),
        }
    }

    pub fn or_insert(self, value: V) -> &'a mut V {
        match self {
            SliceMapEntry::Occupied(entry) => entry.into_mut(),
            SliceMapEntry::Vacant(entry) => entry.insert(value),
        }
    }

    pub fn or_insert_with(
        self,
        value: impl FnOnce(&SliceMapEntryVacant<'a, K, V, S>) -> V,
    ) -> &'a mut V {
        match self {
            SliceMapEntry::Occupied(entry) => entry.into_mut(),
            SliceMapEntry::Vacant(entry) => {
                let value = value(&entry);
                entry.insert(value)
            }
        }
    }

    pub fn and_modify(mut self, f: impl FnOnce(&mut SliceMapEntryOccupied<'a, K, V, S>)) -> Self {
        if let SliceMapEntry::Occupied(entry) = &mut self {
            f(entry);
        }
        self
    }
}

pub struct SliceMapEntryOccupied<'a, K, V, S> {
    _ty: PhantomData<&'a S>,
    key_buf: &'a [K],
    key_range: Range<usize>,
    value: &'a mut V,
}

impl<'a, K, V, S> SliceMapEntryOccupied<'a, K, V, S> {
    // === Key Getters === //

    pub fn key_range(&self) -> Range<usize> {
        self.key_range.clone()
    }

    pub fn key_buf(&self) -> &[K] {
        &self.key_buf
    }

    pub fn key(&self) -> &[K] {
        &self.key_buf[self.key_range()]
    }

    pub fn into_key_buf(self) -> &'a [K] {
        self.key_buf
    }

    pub fn into_key(self) -> &'a [K] {
        &self.key_buf[self.key_range()]
    }

    // === Value Getters === //

    pub fn get(&self) -> &V {
        &self.value
    }

    pub fn get_mut(&mut self) -> &mut V {
        &mut self.value
    }

    pub fn into_mut(self) -> &'a mut V {
        self.value
    }

    // === Composite Getters === //

    pub fn get_key_value(&mut self) -> (&[K], &mut V) {
        (&self.key_buf[self.key_range()], self.value)
    }

    pub fn get_buf_key_value(&mut self) -> (&[K], &[K], &mut V) {
        (self.key_buf, &self.key_buf[self.key_range()], self.value)
    }

    pub fn into_key_value(self) -> (&'a [K], &'a mut V) {
        (&self.key_buf[self.key_range()], self.value)
    }

    pub fn into_buf_key_value(self) -> (&'a [K], &'a [K], &'a mut V) {
        (self.key_buf, &self.key_buf[self.key_range()], self.value)
    }

    pub fn insert(&mut self, value: V) -> V {
        mem::replace(&mut self.value, value)
    }
}

pub struct SliceMapEntryVacant<'a, K, V, S>(SliceMapEntryVacantInner<'a, K, V, S>);

struct SliceMapEntryVacantInner<'a, K, V, S> {
    key_buf: &'a mut Vec<K>,
    retain_len: usize,
    hash: u64,
    entry: hash_map::RawVacantEntryMut<'a, Key, V, S>,
}

impl<'a, K, V, S> SliceMapEntryVacant<'a, K, V, S> {
    fn into_inner(self) -> SliceMapEntryVacantInner<'a, K, V, S> {
        smuggle_drop(self, |v| &v.0)
    }

    // === Insertion === //

    pub fn insert_entry(self, value: V) -> SliceMapEntryOccupied<'a, K, V, S> {
        let me = self.into_inner();
        let (_, value) = me.entry.insert_with_hasher(
            me.hash,
            Key {
                hash: me.hash,
                from: me.retain_len,
                to: me.key_buf.len(),
            },
            value,
            |k| k.hash,
        );

        SliceMapEntryOccupied {
            _ty: PhantomData,
            key_buf: me.key_buf,
            key_range: me.retain_len..me.key_buf.len(),
            value,
        }
    }

    pub fn insert_buf_key_value(self, value: V) -> (&'a [K], &'a [K], &'a mut V) {
        self.insert_entry(value).into_buf_key_value()
    }

    pub fn insert_key_value(self, value: V) -> (&'a [K], &'a mut V) {
        self.insert_entry(value).into_key_value()
    }

    pub fn insert(self, value: V) -> &'a mut V {
        self.insert_entry(value).into_mut()
    }

    // === Key Getters === //

    pub fn key_range(&self) -> Range<usize> {
        self.0.retain_len..self.0.key_buf.len()
    }

    pub fn key_buf(&self) -> &[K] {
        &self.0.key_buf
    }

    pub fn key(&self) -> &[K] {
        &self.0.key_buf[self.key_range()]
    }

    pub fn into_key_buf(self) -> &'a [K] {
        self.into_inner().key_buf
    }

    pub fn into_key(self) -> &'a [K] {
        let retain_len = self.0.retain_len;
        &self.into_inner().key_buf[retain_len..]
    }
}

impl<K, V, S> Drop for SliceMapEntryVacant<'_, K, V, S> {
    fn drop(&mut self) {
        self.0.key_buf.truncate(self.0.retain_len);
    }
}

// === SliceMap Iterators === //

#[derive_where(Clone)]
#[iterator(
    (&'a [K], &'a V),
    self.iter.next().map(|(k, v)| (&self.buf[k.range()], v))
)]
pub struct SliceMapIter<'a, K, V> {
    buf: &'a [K],
    iter: hash_map::Iter<'a, Key, V>,
}

#[iterator(
    (&'a [K], &'a mut V),
    self.iter.next().map(|(k, v)| (&self.buf[k.range()], v))
)]
pub struct SliceMapIterMut<'a, K, V> {
    buf: &'a [K],
    iter: hash_map::IterMut<'a, Key, V>,
}

#[iterator(&'a [K], self.iter.next().map(|k| &self.buf[k.range()]))]
pub struct SliceMapKeys<'a, K, V> {
    buf: &'a [K],
    iter: hash_map::Keys<'a, Key, V>,
}

#[iterator(&'a V, self.iter.next())]
pub struct SliceMapValues<'a, V> {
    iter: hash_map::Values<'a, Key, V>,
}

#[iterator(&'a mut V, self.iter.next())]
pub struct SliceMapValuesMut<'a, V> {
    iter: hash_map::ValuesMut<'a, Key, V>,
}

// === StrMap === //

#[derive_where(Default; S)]
#[derive(Clone)]
pub struct StrMap<V, S> {
    inner: SliceMap<u8, V, S>,
}

impl<V: fmt::Debug, S> fmt::Debug for StrMap<V, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

impl<V, S> StrMap<V, S> {
    pub fn new() -> Self
    where
        S: Default,
    {
        Self::default()
    }

    pub fn with_hasher(hash_builder: S) -> Self {
        Self {
            inner: SliceMap::with_hasher(hash_builder),
        }
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn iter(&self) -> StrMapIter<'_, V> {
        StrMapIter(self.inner.iter())
    }

    pub fn iter_mut(&mut self) -> StrMapIterMut<'_, V> {
        StrMapIterMut(self.inner.iter_mut())
    }

    pub fn keys(&self) -> StrMapKeys<'_, V> {
        StrMapKeys(self.iter())
    }

    pub fn values(&self) -> StrMapValues<'_, V> {
        StrMapValues(self.inner.values())
    }

    pub fn values_mut(&mut self) -> StrMapValuesMut<'_, V> {
        StrMapValuesMut(self.inner.values_mut())
    }
}

impl<V, S> StrMap<V, S>
where
    S: hash::BuildHasher,
{
    pub fn insert(&mut self, key: &str, value: V) -> Option<V> {
        self.inner.insert(key.as_bytes().iter().copied(), value)
    }

    pub fn get(&self, key: &str) -> Option<&V> {
        self.inner.get(key.as_bytes())
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut V> {
        self.inner.get_mut(key.as_bytes())
    }

    pub fn contains(&self, key: &str) -> bool {
        self.inner.contains(key.as_bytes())
    }
}

impl<'a, V, S> IntoIterator for &'a StrMap<V, S> {
    type Item = (&'a str, &'a V);
    type IntoIter = StrMapIter<'a, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, V, S> IntoIterator for &'a mut StrMap<V, S> {
    type Item = (&'a str, &'a mut V);
    type IntoIter = StrMapIterMut<'a, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

// === StrMap Iterators === //

#[derive_where(Clone)]
#[iterator(
    (&'a str, &'a V),
    self.0.next().map(|(k, v)| (unsafe { std::str::from_utf8_unchecked(k) }, v))
)]
pub struct StrMapIter<'a, V>(SliceMapIter<'a, u8, V>);

#[iterator(
    (&'a str, &'a mut V),
    self.0.next().map(|(k, v)| (unsafe { std::str::from_utf8_unchecked(k) }, v))
)]
pub struct StrMapIterMut<'a, V>(SliceMapIterMut<'a, u8, V>);

#[iterator(&'a str, self.0.next().map(|(k, _v)| k))]
pub struct StrMapKeys<'a, V>(StrMapIter<'a, V>);

#[iterator(&'a V, self.0.next())]
pub struct StrMapValues<'a, V>(SliceMapValues<'a, V>);

#[iterator(&'a mut V, self.0.next())]
pub struct StrMapValuesMut<'a, V>(SliceMapValuesMut<'a, V>);
