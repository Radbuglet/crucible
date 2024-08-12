use std::{fmt, hash, ops::Range};

use derive_where::derive_where;
use hashbrown::{hash_map, HashMap};

#[derive_where(Default; S)]
#[derive(Clone)]
pub struct StrMap<V, S> {
    buf: String,
    map: HashMap<Key, V, S>,
}

impl<V: fmt::Debug, S> fmt::Debug for StrMap<V, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_map()
            .entries(self.map.iter().map(|(k, v)| (&self.buf[k.range()], v)))
            .finish()
    }
}

impl<V, S: hash::BuildHasher> StrMap<V, S> {
    pub fn new() -> Self
    where
        S: Default,
    {
        Self::default()
    }

    pub fn with_hasher(hash_builder: S) -> Self {
        Self {
            buf: String::new(),
            map: HashMap::with_hasher(hash_builder),
        }
    }

    pub fn insert(&mut self, key: &str, value: V) -> Option<V> {
        let hash = self.map.hasher().hash_one(key);

        match self
            .map
            .raw_entry_mut()
            .from_hash(hash, |k| k.hash == hash && &self.buf[k.range()] == key)
        {
            hash_map::RawEntryMut::Occupied(mut entry) => Some(entry.insert(value)),
            hash_map::RawEntryMut::Vacant(entry) => {
                let from = self.buf.len();
                self.buf.push_str(key);
                let key = Key {
                    hash,
                    from,
                    to: self.buf.len(),
                };
                entry.insert_with_hasher(hash, key, value, |v| v.hash);
                None
            }
        }
    }

    pub fn get(&self, key: &str) -> Option<&V> {
        let hash = self.map.hasher().hash_one(key);

        self.map
            .raw_entry()
            .from_hash(hash, |k| &self.buf[k.range()] == key)
            .map(|(_k, v)| v)
    }
}

#[derive(Copy, Clone)]
struct Key {
    hash: u64,
    from: usize,
    to: usize,
}

impl fmt::Debug for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}..{}", self.from, self.to)
    }
}

impl Key {
    fn range(self) -> Range<usize> {
        self.from..self.to
    }
}
