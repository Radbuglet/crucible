use std::{
    any::{type_name, TypeId},
    ops::Range,
};

use crucible_utils::{
    define_index,
    hash::{FxHashMap, FxSliceMap},
    newtypes::IndexVec,
};
use derive_where::derive_where;

// === ArchetypeManager === //

pub struct ArchetypeManager {
    archetypes: IndexVec<ArchetypeId, Archetype>,
    archetype_map: FxSliceMap<ComponentId, ArchetypeId>,
}

struct Archetype {
    comp_range: Range<usize>,
    extensions: FxHashMap<TypeId, ArchetypeId>,
    de_extensions: FxHashMap<TypeId, ArchetypeId>,
}

impl ArchetypeManager {
    pub fn new() -> Self {
        let mut manager = Self {
            archetypes: IndexVec::from_iter([Archetype {
                comp_range: 0..0,
                extensions: FxHashMap::default(),
                de_extensions: FxHashMap::default(),
            }]),
            archetype_map: FxSliceMap::default(),
        };
        manager.archetype_map.insert([], ArchetypeId(0));
        manager
    }
}

// === ArchetypeId === //

define_index! {
    pub struct ArchetypeId: u32;
}

// === ComponentId === //

#[derive(Debug, Copy, Clone)]
#[derive_where(Hash, Eq, PartialEq)]
pub struct ComponentId {
    pub id: TypeId,
    #[derive_where(skip)]
    pub name: &'static str,
}

impl ComponentId {
    pub fn of<T: 'static>() -> Self {
        Self {
            id: TypeId::of::<T>(),
            name: type_name::<T>(),
        }
    }
}
