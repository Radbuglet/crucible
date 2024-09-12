use std::{cell::UnsafeCell, fmt};

use crucible_utils::{
    hash::{hashbrown::hash_map, new_fx_hash_map, FxHashMap},
    newtypes::{Arena, Handle},
};
use derive_where::derive_where;

use crate::{ArchetypeId, Entity, EntityLocation, StorageBase};

// === StorageRandHandle === //

#[derive_where(Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct StorageRandHandle<T>(Handle<Slot<T>>);

impl<T> fmt::Debug for StorageRandHandle<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

// === StorageRand === //

pub struct StorageRand<T> {
    arena: Arena<Slot<T>>,
    entity_map: FxHashMap<Entity, StorageRandHandle<T>>,
    archetypes: FxHashMap<ArchetypeId, Vec<StorageRandHandle<T>>>,
}

struct Slot<T> {
    entity: Entity,
    value: UnsafeCell<T>,
}

impl<T: fmt::Debug> fmt::Debug for StorageRand<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let entries = self
            .arena
            .iter()
            .map(|(k, v)| ((v.entity, k), unsafe { &*v.value.get() }));
        f.debug_map().entries(entries).finish()
    }
}

impl<T> Default for StorageRand<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> StorageRand<T> {
    pub const fn new() -> Self {
        Self {
            arena: Arena::new(),
            entity_map: new_fx_hash_map(),
            archetypes: new_fx_hash_map(),
        }
    }
}

unsafe impl<T> StorageBase for StorageRand<T> {
    type Component = T;
    type Handle = StorageRandHandle<T>;

    fn insert(me: &mut Self, entity: Entity, value: Self::Component) -> Self::Handle {
        let entry = match me.entity_map.entry(entity) {
            hash_map::Entry::Vacant(entry) => entry,
            hash_map::Entry::Occupied(entry) => {
                let handle = *entry.get();
                *me.arena[handle.0].value.get_mut() = value;
                return handle;
            }
        };

        let handle = StorageRandHandle(me.arena.insert(Slot {
            entity,
            value: UnsafeCell::new(value),
        }));

        entry.insert(handle);

        handle
    }

    fn remove_entity(
        me: &mut Self,
        entity: Entity,
        location: Option<EntityLocation>,
    ) -> Self::Component {
        if let Some(EntityLocation { archetype, slot }) = location {
            me.archetypes.get_mut(&archetype).unwrap().swap_remove(slot);
        }

        let handle = me.entity_map.remove(&entity).unwrap();
        me.arena.remove(handle.0).unwrap().value.into_inner()
    }

    fn remove_handle(
        me: &mut Self,
        handle: Self::Handle,
        location: Option<EntityLocation>,
    ) -> Self::Component {
        if let Some(EntityLocation { archetype, slot }) = location {
            me.archetypes.get_mut(&archetype).unwrap().swap_remove(slot);
        }

        let entry = me.arena.remove(handle.0).unwrap();
        me.entity_map.remove(&entry.entity);

        entry.value.into_inner()
    }

    fn reshape(me: &mut Self, entity: Entity, src: Option<EntityLocation>, dst: ArchetypeId) {
        let handle = match src {
            Some(EntityLocation { archetype, slot }) => {
                me.archetypes.get_mut(&archetype).unwrap().swap_remove(slot)
            }
            None => me.entity_map.remove(&entity).unwrap(),
        };

        me.archetypes.entry(dst).or_default().push(handle);
    }

    fn reshape_extend(
        me: &mut Self,
        archetype: ArchetypeId,
        entities: impl IntoIterator<Item = Entity>,
    ) {
        me.archetypes
            .entry(archetype)
            .or_default()
            .extend(entities.into_iter().map(|entity| me.entity_map[&entity]));
    }

    fn arch_handles(me: &Self, arch: ArchetypeId) -> impl Iterator<Item = Self::Handle> {
        me.archetypes
            .get(&arch)
            .into_iter()
            .flat_map(|v| v.iter().copied())
    }

    fn arch_values(me: &Self, arch: ArchetypeId) -> impl Iterator<Item = *mut Self::Component> {
        Self::arch_handles(me, arch).map(|handle| me.arena[handle.0].value.get())
    }

    fn arch_values_and_handles(
        me: &Self,
        arch: ArchetypeId,
    ) -> impl Iterator<Item = (Self::Handle, *mut Self::Component)> {
        Self::arch_handles(me, arch).map(|handle| (handle, me.arena[handle.0].value.get()))
    }

    fn entity_to_handle(me: &Self, entity: Entity) -> Option<Self::Handle> {
        me.entity_map.get(&entity).copied()
    }

    fn handle_to_entity(me: &Self, handle: Self::Handle) -> Option<Entity> {
        me.arena.get(handle.0).map(|v| v.entity)
    }

    fn entity_to_value(me: &Self, entity: Entity) -> Option<*mut Self::Component> {
        me.entity_map
            .get(&entity)
            .map(|handle| me.arena[handle.0].value.get())
    }

    fn handle_to_value(me: &Self, handle: Self::Handle) -> Option<*mut Self::Component> {
        me.arena.get(handle.0).map(|v| v.value.get())
    }

    fn entity_to_handle_and_value(
        me: &Self,
        entity: Entity,
    ) -> Option<(Self::Handle, *mut Self::Component)> {
        me.entity_map
            .get(&entity)
            .map(|&handle| (handle, me.arena[handle.0].value.get()))
    }

    fn handle_to_entity_and_value(
        me: &Self,
        handle: Self::Handle,
    ) -> Option<(Entity, *mut Self::Component)> {
        me.arena
            .get(handle.0)
            .map(|slot| (slot.entity, slot.value.get()))
    }
}
