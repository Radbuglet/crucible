use std::ops::Range;

use crucible_utils::{
    define_index,
    hash::{FxHashMap, FxSliceMap},
    iter::{DedupSortedIter, MergeSortedIter, RemoveSortedIter},
    newtypes::IndexVec,
};

use super::{ComponentId, ErasedBundle};

// === ArchetypeManager === //

#[derive(Debug)]
pub struct ArchetypeManager {
    archetypes: IndexVec<ArchetypeId, Archetype>,
    archetype_map: FxSliceMap<ComponentId, ArchetypeId>,
}

#[derive(Debug)]
struct Archetype {
    comp_range: Range<usize>,
    extensions: FxHashMap<ErasedBundle, ArchetypeId>,
    de_extensions: FxHashMap<ErasedBundle, ArchetypeId>,
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
        manager.archetype_map.insert([], ArchetypeId::EMPTY);
        manager
    }

    fn find_generic<M: FindGenericMode>(
        &mut self,
        start: ArchetypeId,
        bundle: ErasedBundle,
    ) -> ArchetypeId {
        // See whether the bundle extension shortcut is already present.
        let start_arch = &mut self.archetypes[start];
        if let Some(extension) = M::pos(start_arch).get(&bundle) {
            return *extension;
        }

        // See whether the archetype already exists.
        let components = DedupSortedIter::new(M::make_iter(
            &self.archetype_map.key_buf()[start_arch.comp_range.clone()],
            bundle.normalized(),
        ));

        if let Some(&end) = self.archetype_map.get(components.clone()) {
            // Add the shortcut.
            M::pos(&mut self.archetypes[start]).insert(bundle, end);
            M::neg(&mut self.archetypes[end]).insert(bundle, start);

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
        M::pos(&mut self.archetypes[start]).insert(bundle, end);
        M::neg(&mut self.archetypes[end]).insert(bundle, start);

        end
    }

    pub fn find_extension(&mut self, start: ArchetypeId, bundle: ErasedBundle) -> ArchetypeId {
        self.find_generic::<FindInsertMode>(start, bundle)
    }

    pub fn find_de_extension(&mut self, start: ArchetypeId, bundle: ErasedBundle) -> ArchetypeId {
        self.find_generic::<FindRemoveMode>(start, bundle)
    }

    pub fn components_of(&self, id: ArchetypeId) -> &[ComponentId] {
        &self.archetype_map.key_buf()[self.archetypes[id].comp_range.clone()]
    }
}

trait FindGenericMode {
    fn pos(arch: &mut Archetype) -> &mut FxHashMap<ErasedBundle, ArchetypeId>;

    fn neg(arch: &mut Archetype) -> &mut FxHashMap<ErasedBundle, ArchetypeId>;

    fn make_iter<'a>(
        source: &'a [ComponentId],
        bundle: &'a [ComponentId],
    ) -> impl Clone + Iterator<Item = &'a ComponentId>;
}

struct FindInsertMode;

impl FindGenericMode for FindInsertMode {
    fn pos(arch: &mut Archetype) -> &mut FxHashMap<ErasedBundle, ArchetypeId> {
        &mut arch.extensions
    }

    fn neg(arch: &mut Archetype) -> &mut FxHashMap<ErasedBundle, ArchetypeId> {
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
    fn pos(arch: &mut Archetype) -> &mut FxHashMap<ErasedBundle, ArchetypeId> {
        &mut arch.de_extensions
    }

    fn neg(arch: &mut Archetype) -> &mut FxHashMap<ErasedBundle, ArchetypeId> {
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

// ArchetypeId
define_index! {
    pub struct ArchetypeId: u32;
}

impl ArchetypeId {
    pub const EMPTY: ArchetypeId = ArchetypeId(0);
}

// EntityLocation
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct EntityLocation {
    pub archetype: ArchetypeId,
    pub slot: usize,
}
