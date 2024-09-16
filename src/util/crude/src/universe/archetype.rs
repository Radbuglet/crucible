use std::{any::TypeId, fmt, ops::Range, sync::OnceLock};

use crucible_utils::{
    define_index,
    hash::{FxHashMap, FxSliceMap},
    iter::{DedupSortedIter, MergeSortedIter, RemoveSortedIter},
    mem::Splicer,
    newtypes::IndexVec,
};
use dashmap::DashMap;

// === ArchetypeManager === //

pub struct ArchetypeManager {
    archetypes: IndexVec<ArchetypeId, Archetype>,
    archetype_map: FxSliceMap<ComponentId, ArchetypeId>,
}

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
        manager.archetype_map.insert([], ArchetypeId(0));
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

// ComponentId
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct ComponentId(TypeId);

impl ComponentId {
    pub fn of<T: 'static>() -> Self {
        Self(TypeId::of::<T>())
    }
}

// === Bundles === //

#[derive(Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct ErasedBundle(fn(&mut Vec<ComponentId>));

impl fmt::Debug for ErasedBundle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReifiedBundle").finish_non_exhaustive()
    }
}

impl ErasedBundle {
    pub fn of<B: Bundle>() -> Self {
        Self(B::write_component_list)
    }

    pub fn normalized(self) -> &'static [ComponentId] {
        static CACHE: OnceLock<DashMap<ErasedBundle, &'static [ComponentId]>> = OnceLock::new();

        let cache = CACHE.get_or_init(Default::default);

        if let Some(cached) = cache.get(&self) {
            return *cached;
        }

        *cache.entry(self).or_insert_with(|| {
            let mut components = Vec::new();
            self.0(&mut components);
            components.sort();

            let mut splicer = Splicer::new(&mut components);
            loop {
                let remaining = splicer.remaining();
                let Some((first_dup_idx, _)) = remaining
                    .windows(2)
                    .enumerate()
                    .find(|(_, win)| win[0] == win[1])
                else {
                    break;
                };

                let first_dup = remaining[first_dup_idx];
                let after_dup = &remaining[first_dup_idx..][1..];
                let dup_seq_len = after_dup
                    .iter()
                    .enumerate()
                    .find(|(_, &other)| first_dup != other)
                    .map_or(after_dup.len(), |v| v.0);

                splicer.splice(first_dup_idx, dup_seq_len, &[]);
            }
            drop(splicer);

            Box::leak(components.into_boxed_slice())
        })
    }
}

pub trait Bundle {
    fn write_component_list(target: &mut Vec<ComponentId>);
}
