use core::fmt;
use std::{
    any::type_name,
    cell::RefCell,
    hash,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    ptr::NonNull,
    sync::Arc,
};

use crucible_utils::{fmt::CowDisplay, hash::FxHashMap, impl_tuples, mem::CellVec};

use crate::{
    ArchetypeId, Bundle, Component, Entity, EntityAllocator, EntityLocation, ErasedBundle, Obj,
};

// === Traits === //

pub struct InsertionResultGeneric<'a, S: Storage> {
    pub old_value: Option<S::Component>,
    pub new_value: &'a mut S::Component,
    pub handle: S::Handle,
}

/// A storage is a three-way map between [`Entity`] handles, a storage-defined
/// [`Handle`](StorageBase::Handle), and the corresponding [`Component`](StorageBase::Component) value.
///
/// A given entity can be either *missing*, *unhoused*, or *housed*.  A *housed* entity exists in a
/// given [`ArchetypeId`] at the specified `slot` and shows up in calls to [`arch_handles`](StorageBase::arch_handles)
/// and the like. An *unhoused* entity, meanwhile, exists in no archetype and will not show up in
/// any calls to `arch_handles` or the like. However, an unhoused entity still has a `handle` and a
/// `value` associated with it.  Naturally, a *missing* entity is neither present in the archetype
/// table nor does it have a `handle` and `value` associated with it.
///
/// The lifecycle of an entity is as follows:
///
/// - [`insert`](StorageBase::insert) on a *missing* entity associates a handle and value with the
///   entity and makes the entity *unhoused*.
/// - [`reshape`](StorageBase::reshape) on an *unhoused* entity makes the entity *housed*.
/// - [`remove_entity`](StorageBase::remove_entity) on an *unhoused* or *housed* entity makes it
///   *missing*.
///
/// Archetypes among all storages contained in a given [`Universe`](crate::Universe) are expected to
/// all share the same order of entities. We achieve this consistent ordering by ensuring that all
/// archetype reshape operations follow the same algorithm:
///
/// - To remove an entity from an archetype, we swap-remove it from the list.
/// - To add an entity to an archetype, we push it to the end.
///
/// ## Safety
///
/// The implementor must satisfy each methods' "contract safety" section.
///
pub unsafe trait Storage: Sized {
    /// The type of value stored in this storage.
    type Component;

    /// The type of the handle used to access these values.
    type Handle: fmt::Debug + Copy + hash::Hash + Ord;

    /// Fetches the friendly-name of the component being stored in this storage.
    fn friendly_name() -> impl fmt::Display {
        type_name::<Self::Component>()
    }

    /// Inserts the given component `value` into the storage, associating it with `entity` and the
    /// returned handle immediately. If the entity was previously *missing*, it will become *unhoused.
    /// Otherwise, it will stay in whatever state it was previously.
    fn insert(
        me: &mut Self,
        entity: Entity,
        value: Self::Component,
    ) -> InsertionResultGeneric<'_, Self>;

    /// Removes the entity from the storage, disassociating it from its `handle`, its `value`, and
    /// whatever archetypal state it had. In other words, it transitions the `entity` to the *missing*
    /// state. `location` is set *iff* the entity is *housed* and points to the entity's location.
    fn remove_entity(
        me: &mut Self,
        entity: Entity,
        location: Option<EntityLocation>,
    ) -> Self::Component;

    /// Removes the `handle` from the storage, disassociating it from its `entity`, its `value`, and
    /// whatever archetypal state it had. In other words, it transitions the `entity` to the *missing*
    /// state. `location` is set *iff* the entity is *housed* and points to the entity's location.
    fn remove_handle(
        me: &mut Self,
        handle: Self::Handle,
        location: Option<EntityLocation>,
    ) -> Self::Component;

    /// Move a non-*missing* entity which was previously in the `src` archetype to the `dst` archetype.
    /// If the `entity` was *housed*, `src` will point to its original location—otherwise, `src` will
    /// be `None`. This method is guaranteed to never be called with an identical archetype ID in
    /// both the `src` and the `dst`.
    fn reshape(me: &mut Self, entity: Entity, src: Option<EntityLocation>, dst: ArchetypeId);

    /// Inserts a list of entities *unhoused* entities in order to the end of the specified `archetype`.
    fn reshape_extend(
        me: &mut Self,
        archetype: ArchetypeId,
        entities: impl IntoIterator<Item = Entity>,
    );

    /// Iterates through the handles in a given archetype in slot order.
    fn arch_handles(me: &Self, arch: ArchetypeId) -> impl Iterator<Item = Self::Handle>;

    /// Iterates through the values in a given archetype in slot order.
    ///
    /// ## Contract Safety
    ///
    /// The pointers returned must be dereferenceable for the duration of the borrow to `me`.
    fn arch_values(me: &Self, arch: ArchetypeId) -> impl Iterator<Item = *mut Self::Component>;

    /// Zips the iterators of both [`arch_handles`](StorageBase::arch_handles) and [`arch_values`](StorageBase::arch_values).
    ///
    /// ## Contract Safety
    ///
    /// The pointers returned must be dereferenceable for the duration of the borrow to `me`.
    fn arch_values_and_handles(
        me: &Self,
        arch: ArchetypeId,
    ) -> impl Iterator<Item = (Self::Handle, *mut Self::Component)>;

    /// Maps an entity to its corresponding handle if it exists in the storage.
    fn entity_to_handle(me: &Self, entity: Entity) -> Option<Self::Handle>;

    /// Maps a handle to its corresponding entity in the storage if it exists.
    fn handle_to_entity(me: &Self, handle: Self::Handle) -> Option<Entity>;

    /// Converts an entity handle into a value thereto.
    ///
    /// ## Safety
    ///
    /// The pointer returned must be dereferenceable for the duration of the borrow to `me`.
    fn entity_to_value(me: &Self, entity: Entity) -> Option<*mut Self::Component>;

    /// Converts a storage handle into a value thereto.
    ///
    /// ## Safety
    ///
    /// The pointer returned must be dereferenceable for the duration of the borrow to `me`.
    fn handle_to_value(me: &Self, handle: Self::Handle) -> Option<*mut Self::Component>;

    /// Converts an entity handle into both a handle and a value thereto.
    ///
    /// ## Safety
    ///
    /// The pointer returned must be dereferenceable for the duration of the borrow to `me`.
    fn entity_to_handle_and_value(
        me: &Self,
        entity: Entity,
    ) -> Option<(Self::Handle, *mut Self::Component)>;

    /// Converts a storage handle into both an entity handle and a value thereto.
    ///
    /// ## Safety
    ///
    /// The pointer returned must be dereferenceable for the duration of the borrow to `me`.
    fn handle_to_entity_and_value(
        me: &Self,
        handle: Self::Handle,
    ) -> Option<(Entity, *mut Self::Component)>;
}

// === ChangeQueue === //

#[derive(Debug, Default)]
pub struct ChangeQueue {
    removed_entities: CellVec<Entity>,
    removed_components: CellVec<(Entity, ErasedBundle)>,
    added_components: CellVec<(Entity, ErasedBundle)>,
    added_components_de_novo: RefCell<FxHashMap<ErasedBundle, Arc<CellVec<Entity>>>>,
}

impl ChangeQueue {
    pub fn push_destroy(&self, target: Entity) {
        self.removed_entities.push(target);
    }

    pub fn push_component_remove(&self, target: Entity, bundle: ErasedBundle) {
        self.removed_components.push((target, bundle));
    }

    pub fn push_component_add(&self, target: Entity, bundle: ErasedBundle) {
        self.added_components.push((target, bundle));
    }

    pub fn components_de_novo(&self, bundle: ErasedBundle) -> DeNovoChangeQueue<'_> {
        DeNovoChangeQueue {
            _ty: PhantomData,
            value: self
                .added_components_de_novo
                .borrow_mut()
                .entry(bundle)
                .or_default()
                .clone(),
        }
    }

    pub fn into_inner(self) -> ChangeQueueFinished {
        ChangeQueueFinished {
            removed_entities: self.removed_entities.finish(),
            removed_components: self.removed_components.finish(),
            added_components: self.added_components.finish(),
            added_components_de_novo: self
                .added_components_de_novo
                .into_inner()
                .into_iter()
                .map(|(k, v)| (k, Arc::into_inner(v).unwrap().finish()))
                .collect(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DeNovoChangeQueue<'a> {
    _ty: PhantomData<&'a ChangeQueue>,
    value: Arc<CellVec<Entity>>,
}

impl<'a> DeNovoChangeQueue<'a> {
    pub fn erase(self) -> DeNovoChangeQueue<'static> {
        DeNovoChangeQueue {
            _ty: PhantomData,
            value: self.value,
        }
    }

    pub fn push(&self, value: Entity) {
        self.value.push(value);
    }
}

#[derive(Debug, Default)]
pub struct ChangeQueueFinished {
    pub removed_entities: Vec<Entity>,
    pub removed_components: Vec<(Entity, ErasedBundle)>,
    pub added_components: Vec<(Entity, ErasedBundle)>,
    pub added_components_de_novo: FxHashMap<ErasedBundle, Vec<Entity>>,
}

// === StorageViewModify === //

pub trait StorageViewModify {
    type Values: Bundle;
    type Results;

    // === Required methods === //

    fn queue(&self) -> &ChangeQueue;

    fn insert_no_record(&mut self, entity: Entity, values: Self::Values) -> Self::Results;

    // === Public Surface === //

    fn spawn_many<L: CowDisplay>(
        &mut self,
        alloc: &mut EntityAllocator,
        entries: impl IntoIterator<Item = (L, Self::Values)>,
    ) -> impl Iterator<Item = Self::Results> {
        let queue = self
            .queue()
            .components_de_novo(ErasedBundle::of::<Self::Values>())
            .erase();

        entries.into_iter().map(move |(label, values)| {
            let entity = alloc.spawn(label);
            queue.push(entity);
            self.insert_no_record(entity, values)
        })
    }

    fn spawn_arr<L: CowDisplay, const N: usize>(
        &mut self,
        alloc: &mut EntityAllocator,
        entries: [(L, Self::Values); N],
    ) -> [Self::Results; N] {
        let queue = self
            .queue()
            .components_de_novo(ErasedBundle::of::<Self::Values>())
            .erase();

        entries.map(|(label, values)| {
            let entity = alloc.spawn(label);
            queue.push(entity);
            self.insert_no_record(entity, values)
        })
    }

    fn spawn(
        &mut self,
        alloc: &mut EntityAllocator,
        label: impl CowDisplay,
        values: Self::Values,
    ) -> Self::Results {
        let [res] = self.spawn_arr(alloc, [(label, values)]);
        res
    }
}

impl<T: Component> StorageViewModify for StorageView<T> {
    type Values = T;
    type Results = Obj<T>;

    fn queue(&self) -> &ChangeQueue {
        self.queue()
    }

    fn insert_no_record(&mut self, entity: Entity, values: Self::Values) -> Obj<T> {
        Obj::from_raw(<T::Storage>::insert(self.storage_mut(), entity, values).handle)
    }
}

impl<'a, T: ?Sized + StorageViewModify> StorageViewModify for &'a mut T {
    type Values = T::Values;
    type Results = T::Results;

    fn queue(&self) -> &ChangeQueue {
        (**self).queue()
    }

    fn insert_no_record(&mut self, entity: Entity, values: Self::Values) -> Self::Results {
        (*self).insert_no_record(entity, values)
    }
}

macro_rules! impl_storage_view_modify {
    ($($param:ident:$field:tt),*) => {
        impl<$($param: StorageViewModify),*> StorageViewModify for ($($param,)*) {
            type Values = ($($param::Values,)*);
            type Results = ($($param::Results,)*);

            fn queue(&self) -> &ChangeQueue {
                self.0.queue()
            }

            fn insert_no_record(&mut self, entity: Entity, values: Self::Values) -> Self::Results {
                ($(self.$field.insert_no_record(entity, values.$field),)*)
            }
        }
    };
}

impl_tuples!(impl_storage_view_modify; no_unit);

// === StorageView === //

pub struct InsertionResult<'a, T: Component> {
    pub old_value: Option<T>,
    pub new_value: &'a mut T,
    pub handle: Obj<T>,
}

// Bare view types
pub struct StorageView<T: Component> {
    queue: NonNull<ChangeQueue>,
    storage: NonNull<T::Storage>,
}

impl<T: Component> StorageView<T> {
    pub fn queue(&self) -> &ChangeQueue {
        unsafe { self.queue.as_ref() }
    }

    pub fn storage(&self) -> &T::Storage {
        unsafe { self.storage.as_ref() }
    }

    pub fn storage_mut(&mut self) -> &mut T::Storage {
        unsafe { self.storage.as_mut() }
    }

    pub fn queue_storage_and_mut(&mut self) -> (&ChangeQueue, &mut T::Storage) {
        let queue = unsafe { self.queue.as_ref() };
        let storage = unsafe { self.storage.as_mut() };

        (queue, storage)
    }
}

#[repr(transparent)]
pub struct StorageViewRef<'a, T: Component> {
    _ty: PhantomData<&'a ()>,
    view: StorageView<T>,
}

impl<'a, T: Component> StorageViewRef<'a, T> {
    pub fn new(queue: &'a ChangeQueue, storage: &'a T::Storage) -> Self {
        Self {
            _ty: PhantomData,
            view: StorageView {
                queue: NonNull::from(queue),
                storage: NonNull::from(storage),
            },
        }
    }
}

impl<'a, T: Component> Deref for StorageViewRef<'a, T> {
    type Target = StorageView<T>;

    fn deref(&self) -> &Self::Target {
        &self.view
    }
}

#[repr(transparent)]
pub struct StorageViewMut<'a, T: Component> {
    _ty: PhantomData<&'a ()>,
    view: StorageView<T>,
}

impl<'a, T: Component> StorageViewMut<'a, T> {
    pub fn new(queue: &'a ChangeQueue, storage: &'a mut T::Storage) -> Self {
        Self {
            _ty: PhantomData,
            view: StorageView {
                queue: NonNull::from(queue),
                storage: NonNull::from(storage),
            },
        }
    }
}

impl<'a, T: Component> Deref for StorageViewMut<'a, T> {
    type Target = StorageView<T>;

    fn deref(&self) -> &Self::Target {
        &self.view
    }
}

impl<'a, T: Component> DerefMut for StorageViewMut<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.view
    }
}

// View API
impl<'a, T: Component> StorageView<T> {
    fn missing_error(handle: impl fmt::Debug) -> ! {
        panic!(
            "{handle:?} is missing component {}",
            <T::Storage>::friendly_name()
        )
    }

    pub fn try_get_entity(&self, entity: Entity) -> Option<&T> {
        <T::Storage>::entity_to_value(&self.storage(), entity).map(|v| unsafe { &*v })
    }

    pub fn try_get_entity_mut(&mut self, entity: Entity) -> Option<&mut T> {
        <T::Storage>::entity_to_value(&self.storage(), entity).map(|v| unsafe { &mut *v })
    }

    pub fn get_entity(&self, entity: Entity) -> &T {
        self.try_get_entity(entity)
            .unwrap_or_else(|| Self::missing_error(entity))
    }

    pub fn get_entity_mut(&mut self, entity: Entity) -> &mut T {
        self.try_get_entity_mut(entity)
            .unwrap_or_else(|| Self::missing_error(entity))
    }

    pub fn try_get(&self, handle: Obj<T>) -> Option<&T> {
        <T::Storage>::handle_to_value(&self.storage(), handle.raw()).map(|v| unsafe { &*v })
    }

    pub fn try_get_mut(&mut self, handle: Obj<T>) -> Option<&mut T> {
        <T::Storage>::handle_to_value(&self.storage(), handle.raw()).map(|v| unsafe { &mut *v })
    }

    pub fn get(&self, handle: Obj<T>) -> &T {
        self.try_get(handle)
            .unwrap_or_else(|| Self::missing_error(handle))
    }

    pub fn get_mut(&mut self, handle: Obj<T>) -> &mut T {
        self.try_get_mut(handle)
            .unwrap_or_else(|| Self::missing_error(handle))
    }
}
