use core::hash;
use std::{fmt, ops::Range};

use crucible_utils_proc::iterator;
use derive_where::derive_where;
use hashbrown::{hash_map, HashMap};

use super::BuildHasherExt as _;
use crate::mem::{defuse, guard};

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
}

impl<K, V, S> SliceMap<K, V, S>
where
    K: hash::Hash + Eq,
    S: hash::BuildHasher,
{
    pub fn insert(&mut self, key: impl IntoIterator<Item = K>, value: V) -> Option<V> {
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

        // Insert the entry.
        match entry {
            hash_map::RawEntryMut::Occupied(mut entry) => Some(entry.insert(value)),
            hash_map::RawEntryMut::Vacant(entry) => {
                let key = Key {
                    hash,
                    from: old_buf_len,
                    to: buf.len(),
                };
                entry.insert_with_hasher(hash, key, value, |v| v.hash);
                defuse(buf);
                None
            }
        }
    }

    pub fn get<'v, KI>(&self, key: KI) -> Option<&V>
    where
        KI: IntoIterator<Item = &'v K> + Clone,
        KI::IntoIter: ExactSizeIterator,
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
        KI::IntoIter: ExactSizeIterator,
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
        KI::IntoIter: ExactSizeIterator,
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
