use std::{
    any::{type_name, TypeId},
    ops::Range,
};

use crucible_utils::{
    define_index,
    hash::{FxHashMap, FxSliceMap},
    iter::{MergeSortedIter, RemoveSortedIter},
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

impl Default for ArchetypeManager {
    fn default() -> Self {
        Self::new()
    }
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

    fn find_generic<M: FindGenericMode>(
        &mut self,
        start: ArchetypeId,
        bundle_ty: TypeId,
        bundle_comps: &[ComponentId],
    ) -> ArchetypeId {
        // See whether the bundle extension shortcut is already present.
        let start_arch = &mut self.archetypes[start];
        if let Some(extension) = M::pos(start_arch).get(&bundle_ty) {
            return *extension;
        }

        // See whether the archetype already exists.
        let components = M::make_iter(
            &self.archetype_map.key_buf()[start_arch.comp_range.clone()],
            bundle_comps,
        );

        if let Some(&end) = self.archetype_map.get(components.clone()) {
            // Add the shortcut.
            M::pos(&mut self.archetypes[start]).insert(bundle_ty, end);
            M::neg(&mut self.archetypes[end]).insert(bundle_ty, start);

            return end;
        }

        // Otherwise, create the archetype.
        let end = self.archetypes.push(Archetype {
            comp_range: 0..0,
            extensions: FxHashMap::default(),
            de_extensions: FxHashMap::default(),
        });
        let components = components.copied().collect::<Vec<_>>();
        let comp_range = self.archetype_map.entry(components).insert(end).key_range();
        self.archetypes[end].comp_range = comp_range;

        // ...and add the shortcut.
        M::pos(&mut self.archetypes[start]).insert(bundle_ty, end);
        M::neg(&mut self.archetypes[end]).insert(bundle_ty, start);

        end
    }

    pub fn find_extension(
        &mut self,
        start: ArchetypeId,
        bundle_ty: TypeId,
        bundle_comps: &[ComponentId],
    ) -> ArchetypeId {
        self.find_generic::<FindInsertMode>(start, bundle_ty, bundle_comps)
    }

    pub fn find_de_extension(
        &mut self,
        start: ArchetypeId,
        bundle_ty: TypeId,
        bundle_comps: &[ComponentId],
    ) -> ArchetypeId {
        self.find_generic::<FindRemoveMode>(start, bundle_ty, bundle_comps)
    }
}

trait FindGenericMode {
    fn pos(arch: &mut Archetype) -> &mut FxHashMap<TypeId, ArchetypeId>;

    fn neg(arch: &mut Archetype) -> &mut FxHashMap<TypeId, ArchetypeId>;

    fn make_iter<'a>(
        source: &'a [ComponentId],
        bundle: &'a [ComponentId],
    ) -> impl Clone + Iterator<Item = &'a ComponentId>;
}

struct FindInsertMode;

impl FindGenericMode for FindInsertMode {
    fn pos(arch: &mut Archetype) -> &mut FxHashMap<TypeId, ArchetypeId> {
        &mut arch.extensions
    }

    fn neg(arch: &mut Archetype) -> &mut FxHashMap<TypeId, ArchetypeId> {
        &mut arch.de_extensions
    }

    fn make_iter<'a>(
        source: &'a [ComponentId],
        bundle: &'a [ComponentId],
    ) -> impl Clone + Iterator<Item = &'a ComponentId> {
        MergeSortedIter::new(source, bundle)
    }
}

struct FindRemoveMode;

impl FindGenericMode for FindRemoveMode {
    fn pos(arch: &mut Archetype) -> &mut FxHashMap<TypeId, ArchetypeId> {
        &mut arch.de_extensions
    }

    fn neg(arch: &mut Archetype) -> &mut FxHashMap<TypeId, ArchetypeId> {
        &mut arch.extensions
    }

    fn make_iter<'a>(
        source: &'a [ComponentId],
        bundle: &'a [ComponentId],
    ) -> impl Clone + Iterator<Item = &'a ComponentId> {
        RemoveSortedIter::new(source, bundle)
    }
}

// === Handles === //

define_index! {
    pub struct ArchetypeId: u32;
}

impl ArchetypeId {
    pub const EMPTY: ArchetypeId = ArchetypeId(0);
}

#[derive(Debug, Copy, Clone)]
#[derive_where(Hash, Eq, PartialEq, Ord, PartialOrd)]
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
