use crate::{ArchetypeId, Entity};

// === Traits === //

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct EntityLocation {
    pub archetype: ArchetypeId,
    pub index: usize,
}

pub trait StorageHandle: Sized + 'static + Copy + Ord {}

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
/// ## Safety
///
/// The implementor must satisfy each methods' "contract safety" section.
///
pub unsafe trait StorageBase {
    /// The type of value stored in this storage.
    type Component;

    /// The type of the handle used to access these values.
    type Handle: StorageHandle;

    /// Inserts the given component `value` into the storage, associating it with `entity` and the
    /// returned handle immediately.
    ///
    /// If the entity was previously *missing*, it will become *unhoused. Otherwise, it will stay in
    /// whatever state it was previously.
    fn insert(me: &mut Self, entity: Entity, value: Self::Component) -> Self::Handle;

    /// Removes the entity from the storage, disassociating it from its `handle`, its `value`, and
    /// whatever archetypal state it had. In other words, it transitions the `entity` to the *missing*
    /// state.
    fn remove_entity(me: &mut Self, entity: Entity) -> Option<Self::Component>;

    /// Removes the `handle` from the storage, disassociating it from its `entity`, its `value`, and
    /// whatever archetypal state it had. In other words, it transitions the `entity` to the *missing*
    /// state.
    fn remove_handle(me: &mut Self, handle: Self::Handle) -> Option<Self::Component>;

    /// Move an entity which was previously in the `src` archetype to the `dst` archetype. A `src`
    /// of `None` indicates that the entity was not previously in the storage.
    fn reshape(me: &mut Self, entity: Entity, src: Option<EntityLocation>, dst: EntityLocation);

    /// Inserts a list of entities which were not previously in the storage into the storage at the
    /// end of the specified `archetype`.
    fn reshape_extend(me: &mut Self, archetype: ArchetypeId, entities: &[Entity]);

    /// Iterates through the handles in a given archetype in slot order.
    fn arch_handles(me: &Self, arch: ArchetypeId) -> impl Iterator<Item = Self::Handle>;

    /// Iterates through the values in a given archetype in slot order.
    ///
    /// ## Contract Safety
    ///
    /// The pointers returned must be dereferenceable for the duration of the borrow to `me`.
    fn arch_values(me: &Self, arch: ArchetypeId) -> impl Iterator<Item = *mut Self::Handle>;

    /// Zips the iterators of both [`arch_handles`](StorageBase::arch_handles) and [`arch_values`](StorageBase::arch_values).
    ///
    /// ## Contract Safety
    ///
    /// The pointers returned must be dereferenceable for the duration of the borrow to `me`.
    fn arch_values_and_handles(
        me: &Self,
        arch: ArchetypeId,
    ) -> impl Iterator<Item = (Self::Handle, *mut Self::Handle)>;

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
        entity: Self::Handle,
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

// === Queries === //

// TODO
