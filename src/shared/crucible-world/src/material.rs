use std::collections::hash_map;

use bevy_ecs::entity::Entity;
use derive_where::derive_where;
use newtypes::{Index, IndexVec};
use rustc_hash::FxHashMap;

#[derive_where(Debug, Default)]
pub struct MaterialRegistry<K: Index> {
    descriptors: IndexVec<K, Entity>,
    name_map: FxHashMap<String, K>,
}

impl<K: Index> MaterialRegistry<K> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, name: impl Into<String>, descriptor: Entity) -> K {
        let entry = match self.name_map.entry(name.into()) {
            hash_map::Entry::Occupied(entry) => {
                tracing::warn!(
                    "multiple material descriptors assigned the name {:?}; ignoring subsequent entry",
                    entry.key()
                );
                return *entry.into_mut();
            }
            hash_map::Entry::Vacant(entry) => entry,
        };

        let idx = self.descriptors.push(descriptor);
        entry.insert(idx);
        idx
    }

    pub fn lookup_by_idx(&self, idx: K) -> Entity {
        self.descriptors[idx]
    }

    pub fn lookup_by_name(&self, name: &str) -> Option<K> {
        self.name_map.get(name).copied()
    }

    pub fn lookup_desc_by_name(&self, name: &str) -> Option<Entity> {
        self.lookup_by_name(name).map(|idx| self.lookup_by_idx(idx))
    }
}
