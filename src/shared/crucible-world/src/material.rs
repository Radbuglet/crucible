use std::collections::hash_map;

use bevy_autoken::{Obj, RandomComponent, RandomEntityExt};
use bevy_ecs::entity::Entity;
use crucible_utils::newtypes::{IndexVec, LargeIndex};
use derive_where::derive_where;
use rustc_hash::FxHashMap;

// === MaterialRegistry === //

#[derive_where(Debug, Default)]
pub struct MaterialRegistry<K: LargeIndex> {
    descriptors: IndexVec<K, Entity>,
    name_map: FxHashMap<String, K>,
}

impl<K: LargeIndex> MaterialRegistry<K> {
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

// === MaterialCache === //

#[derive(Debug)]
pub struct MaterialCache<K: LargeIndex, V> {
    registry: Obj<MaterialRegistry<K>>,
    cache: IndexVec<K, Option<Obj<V>>>,
}

impl<K, V> MaterialCache<K, V>
where
    K: LargeIndex,
    MaterialRegistry<K>: RandomComponent,
    V: RandomComponent,
{
    pub const fn new(registry: Obj<MaterialRegistry<K>>) -> Self {
        Self {
            registry,
            cache: IndexVec::new(),
        }
    }

    pub fn get(&mut self, id: K) -> Option<Obj<V>> {
        match self.cache.entry(id) {
            Some(entry) => Some(*entry),
            v @ None => {
                let descriptor = self.registry.lookup_by_idx(id).try_get::<V>()?;
                *v = Some(descriptor);
                Some(descriptor)
            }
        }
    }
}
