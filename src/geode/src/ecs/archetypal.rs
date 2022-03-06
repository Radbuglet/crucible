// TODO: Use bump allocation, rather than a bunch of vecs, to store archetypes.

use crate::ecs::entity::Entity;
use crate::exec::lock::{RwGuardMut, RwLock, RwLockExt, RwLockManager};
use crate::util::free_list::FreeList;
use crate::util::lifetime::Take;
use crate::util::number::{NonZeroU64Ext, OptionalUsize};
use hashbrown::raw::RawTable;
use smallvec::SmallVec;
use std::any::Any;
use std::cell::UnsafeCell;
use std::fmt::{Debug, Formatter};
use std::hash::{BuildHasher, Hasher};
use std::marker::PhantomData;
use std::num::NonZeroU64;
use std::ptr::NonNull;
use std::sync::Arc;

pub const MAX_IDEAL_COMPONENTS: usize = 8;

type StorageId = NonZeroU64;
type EntityGen = NonZeroU64;

// === World structures === //

type WorldHashBuilder = std::collections::hash_map::RandomState;

#[derive(Clone)]
pub struct World {
    inner: Arc<RwLock<WorldInner>>,
}

struct WorldInner {
    // A monotonically increasing entity generation counter.
    entity_gen_gen: EntityGen,

    // A monotonically increasing storage ID generator.
    // See [ArchStorage.uid] for details.
    store_id_gen: StorageId,

    // A free-list of [EntitySlot]s, which allow users to map an [Entity] to its slot in its
    // containing archetype and to check that it's even still alive.
    entities: FreeList<EntitySlot>,

    // Maps archetype indices to their head.
    archetypes: FreeList<Archetype>,

    // Maps archetype component list hashes to archetype indices.
    // Archetype key equalities are checked using the corresponding [Archetype.components] field.
    comps_to_arch: RawTable<(u64, usize)>,

    // The hash builder for `comps_to_arch`.
    hash_builder: WorldHashBuilder,
}

#[derive(Copy, Clone)]
struct EntitySlot {
    // The entity's generation, used to determine whether an entity is still alive.
    gen: EntityGen,

    // The component's archetype.
    arch_index: OptionalUsize,

    // Entity index within that archetype.
    comp_index: usize,
}

struct Archetype {
    // A list of entity indices within the archetype.
    entities: Vec<usize>,

    // A list of component storages and their monotonically increasing ID.
    components: SmallVec<[(StorageId, Box<dyn ComponentContainer>); MAX_IDEAL_COMPONENTS]>,
}

trait ComponentContainer: Any {
    unsafe fn push(&mut self, from: *mut ());
    unsafe fn get_comp(&self, index: usize) -> *mut ();
    fn swap_and_forget_comp(&mut self, index: usize);
    unsafe fn drop_last_comp(&mut self);
}

impl<T: 'static> ComponentContainer for Vec<UnsafeCell<T>> {
    unsafe fn push(&mut self, from: *mut ()) {
        // N.B. `MaybeUninit` is `repr(transparent)` so this is safe *for us*, regardless of whether
        // the from pointer is wrapped in it.
        self.push(UnsafeCell::new(from.cast::<T>().read()))
    }

    unsafe fn get_comp(&self, index: usize) -> *mut () {
        self.get_unchecked(index).get().cast::<()>()
    }

    fn swap_and_forget_comp(&mut self, index: usize) {
        let last_index = self.len() - 1;

        // Move the target `index` into the `last_index` slot.
        self.swap(index, last_index);

        // And reduce the length by one to remove the element without dropping it.
        unsafe { self.set_len(last_index) }
    }

    unsafe fn drop_last_comp(&mut self) {
        self.spare_capacity_mut()
            .get_unchecked_mut(0)
            .assume_init_drop();
    }
}

impl World {
    pub fn new<M: Take<RwLockManager>>(manager: M) -> Self {
        Self {
            inner: Arc::new(
                WorldInner {
                    entity_gen_gen: NonZeroU64::new(1).unwrap(),
                    store_id_gen: NonZeroU64::new(1).unwrap(),
                    entities: FreeList::new(),
                    archetypes: FreeList::new(),
                    comps_to_arch: RawTable::new(),
                    hash_builder: WorldHashBuilder::new(),
                }
                .wrap_lock(manager),
            ),
        }
    }

    pub fn use_sync(&self) -> WorldImmediate<'_> {
        WorldImmediate {
            world: self,
            guard: self.inner.lock_mut_now(),
        }
    }

    pub fn is_flushed(&self) -> bool {
        self.inner.can_lock_now_mut()
    }

    pub fn debug_assert_flushed(&self) {
        debug_assert!(
            self.is_flushed(),
            "The world has not been flushed: queued world accessors are still alive."
        );
    }
}

impl Eq for World {}

impl PartialEq for World {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

impl Debug for World {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("World")
            .field("inner", &Arc::as_ptr(&self.inner))
            .finish()
    }
}

// === Storages === //

#[derive(Debug)]
pub struct ArchStorage<T> {
    // T is covariant, is inherited during `Send + Sync` determination, and is brought into
    // consideration by the drop checker.
    _ty: PhantomData<T>,

    // The [World] this storage exists in.
    world: World,

    // A world-unique identifier for the storage.
    id: StorageId,

    // A universally-ordered list of archetypes containing this [Storage]. Dead archetypes are
    // cleaned up while walking this list.
    //
    // ## What does "universally-ordered" mean?
    //
    // Universally-ordered, in this context, means that storages will always have some universally-
    // consistent relative position in the list, as if they were sorted by their creation time.
    //
    // When archetypes are created from a list of storages, they add themselves to the end of each
    // storage's `containers` list. Because archetypes never change the list of storages they contain,
    // maintaining this invariant is practically free.
    //
    // This invariant is useful because it allows us to quickly walk along several storages'
    // `containers lists simultaneously to determine which archetypes contain the union of those
    // storages' components.
    containers: Vec<ArchStorageContainer>,
}

#[derive(Debug)]
struct ArchStorageContainer {
    arch_index: usize,
    comp_index: usize,
}

// === Bundles === //

#[derive(Debug, Copy, Clone)]
pub struct ComponentBundleEntry {
    // The ID of the [ArchStorage] into which this component is being inserted.
    pub storage_id: StorageId,

    // A pointer to an initialized `MaybeUninit<T>` from which we can take ownership of the component
    // instance or `None` if the entry is being imported from the existing entity's layout.
    pub take_from_or_import: Option<NonNull<()>>,
}

pub enum ComponentBundleResult<B: ComponentBundle> {
    Keep(B::CompArray),
    Reshape(B::CompArray),
}

pub unsafe trait ComponentBundle: Sized {
    type CompArray: AsRef<[ComponentBundleEntry]>;

    fn to_components_sorted<I>(self, existing: I) -> ComponentBundleResult<Self>
    where
        I: Iterator<Item = StorageId>;
}

// === World accessors === //

pub trait WorldAccessor {
    // === World properties === //
    fn world(&self) -> &World;
    fn is_sync(&self) -> bool;

    // === Entity management === //
    fn spawn(&mut self) -> Entity;
    fn despawn(&mut self, entity: Entity);
    fn is_alive(&self, entity: Entity) -> bool;

    // === Base storage management === //
    fn new_storage<T>(&mut self) -> ArchStorage<T>;
    fn attach_many<B: ComponentBundle>(&mut self, entity: Entity, bundle: B);

    // === Derived storage management === //
    // TODO
}

pub struct WorldImmediate<'a> {
    world: &'a World,
    guard: RwGuardMut<'a, WorldInner>,
}

impl Debug for WorldImmediate<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorldImmediate")
            .field("world", &self.world)
            .finish_non_exhaustive()
    }
}

impl<'a> WorldAccessor for WorldImmediate<'a> {
    fn world(&self) -> &World {
        self.world
    }

    fn is_sync(&self) -> bool {
        true
    }

    fn spawn(&mut self) -> Entity {
        let mut world = self.guard.get();

        // Increment entity generation
        let gen = world
            .entity_gen_gen
            .checked_add_assign(1)
            .expect("Failed to spawn entity: too many entities.");

        // Register the slot
        let slot = world.entities.add(EntitySlot {
            gen,
            arch_index: OptionalUsize::NONE,
            comp_index: 0,
        });

        Entity { slot, gen }
    }

    fn despawn(&mut self, entity: Entity) {
        let mut world_guard = self.guard.get();
        let world = &mut **world_guard;

        // Fetch entity info
        let info = *get_entity_info_mut(&mut world.entities, entity)
            .expect("Attempted to despawn a dead entity.");

        // === Unregister entity === //
        // N.B. We are *very* careful not to call to user code since they could panic and bring the
        // entire ECS into an invalid state.

        // First, we unregister the entity from the entity free list.
        world.entities.release(entity.slot);

        // Then, we fetch the archetype.
        let arch = match info.arch_index.unwrap() {
            Some(index) => &mut world.archetypes[index],
            // If the entity was never registered in an archetype, stop here.
            None => return,
        };

        // Next, we then perform a no-drop swap_remove. This means that we swap the removed element
        // to the last slot of the vec and the vec is resized to effectively forget about the element.
        arch.entities.swap_remove(info.comp_index);

        for (_, comp) in &mut arch.components {
            comp.swap_and_forget_comp(info.comp_index);
        }

        // In swap removing, the entity which used to be at the end of the archetype now has the index
        // `info.comp_index`. Let's ensure that the `comp_index` of the moved entity reflects that.
        if let Some(moved) = arch.entities.get(info.comp_index).copied() {
            world.entities[moved].comp_index = info.comp_index;
        }

        // Finally, now that all the internal state is in order, we can call out to user drop code.
        // Because we haven't touched the component arrays since we "swap_and_forget_comp" then, we
        // can access the first element of the unused capacity slice and drop them there.
        for (_, comp) in &mut arch.components {
            unsafe {
                comp.drop_last_comp();
            }
        }
    }

    fn is_alive(&self, entity: Entity) -> bool {
        get_entity_info(&self.guard.get_ref().entities, entity).is_some()
    }

    fn new_storage<T>(&mut self) -> ArchStorage<T> {
        let mut world = self.guard.get();

        ArchStorage {
            _ty: PhantomData,
            world: self.world.clone(),
            id: world
                .store_id_gen
                .checked_add_assign(1)
                .expect("Too many archetypes!"),
            containers: Vec::new(),
        }
    }

    fn attach_many<B: ComponentBundle>(&mut self, entity: Entity, bundle: B) {
        let mut world_guard = self.guard.get();
        let world = &mut **world_guard;

        // Get the entity's current status.
        let info = get_entity_info_mut(&mut world.entities, entity)
            .expect("Attempted to attach components to a dead entity.");

        // Find out where we need to import our component list from.
        let (arch_info, bundle_instr) = match info.arch_index.unwrap() {
            Some(index) => {
                let arch_info = &mut world.archetypes[index];
                let comps_sorted = bundle.to_components_sorted(
                    arch_info
                        .components
                        .iter()
                        .map(|(storage_id, _)| *storage_id),
                );
                (Some(arch_info), comps_sorted)
            }
            None => (None, bundle.to_components_sorted(std::iter::empty())),
        };

        // Handle special cases
        match bundle_instr {
            ComponentBundleResult::Keep(changed_comps) => {
                let comps = changed_comps.as_ref();

                if let Some(arch_info) = arch_info {
                    debug_assert!(comps.len() == arch_info.components.len());
                    todo!()
                } else {
                    debug_assert!(comps.is_empty());
                    // Nothing to do here.
                }
            }
            ComponentBundleResult::Reshape(comps_opaque) => {
                todo!()
            }
        }
    }
}

fn get_entity_info(entities: &FreeList<EntitySlot>, entity: Entity) -> Option<&EntitySlot> {
    entities
        .get(entity.slot)
        .filter(|slot| slot.gen == entity.gen)
}

fn get_entity_info_mut(
    entities: &mut FreeList<EntitySlot>,
    entity: Entity,
) -> Option<&mut EntitySlot> {
    entities
        .get_mut(entity.slot)
        .filter(|slot| slot.gen == entity.gen)
}

fn hash_bundle(hash_builder: &mut WorldHashBuilder, bundle: &[ComponentBundleEntry]) -> u64 {
    let mut hasher = hash_builder.build_hasher();
    for entry in bundle {
        hasher.write_u64(entry.storage_id.get());
    }
    hasher.finish()
}

fn find_archetype_slot_from_storages(
    archetypes: &mut FreeList<Archetype>,
    storage_map: &mut RawTable<(u64, usize)>,
    bundle: &[ComponentBundleEntry],
    bundle_hash: u64,
) -> Option<usize> {
    storage_map
        .get(bundle_hash, |(candidate_hash, candidate_index)| {
            // Validate full hash
            if *candidate_hash != bundle_hash {
                return false;
            }

            // Validate component list
            let comps = &archetypes[*candidate_index].components;
            bundle.len() == comps.len()
                && bundle
                    .iter()
                    .zip(comps.iter())
                    .all(|(entry, (candidate_gen, _))| entry.storage_id == *candidate_gen)
        })
        .copied()
        .map(|(_, index)| index)
}
