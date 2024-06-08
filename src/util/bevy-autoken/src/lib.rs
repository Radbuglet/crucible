#![allow(clippy::missing_safety_doc)]

use std::{
    cell::Cell,
    collections::hash_map,
    fmt, hash,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    ptr::NonNull,
    thread::LocalKey,
};

use autoken::{cap, Borrows, CapTarget, Ref, TokenSet};
use bevy_app::{App, Last};
use bevy_ecs::{
    bundle::Bundle,
    component::{Component, ComponentId, Tick},
    entity::Entity,
    event::{Event, Events},
    removal_detection::RemovedComponents,
    system::{Commands, In, Res, ResMut, Resource, RunSystemOnce, SystemMeta, SystemParam},
    world::{unsafe_world_cell::UnsafeWorldCell, World},
};
use generational_arena::{Arena, Index};
use rustc_hash::FxHashMap;

// === RandomArena === //

#[derive(Debug, Resource)]
pub struct RandomArena<T> {
    pub arena: Arena<(Entity, T)>,
    pub map: FxHashMap<Entity, Obj<T>>,
}

impl<T> Default for RandomArena<T> {
    fn default() -> Self {
        Self {
            arena: Arena::default(),
            map: FxHashMap::default(),
        }
    }
}

// === RandomAccess === //

mod sealed {
    use super::*;

    cap! {
        pub CommandsCap<'w, 's> = Commands<'w, 's>;
        pub WorldCap<'w> = UnsafeWorldCell<'w>;
    }
}

use sealed::*;

pub struct RandomAccess<'w, 's, L: RandomResourceList> {
    inner: RandomAccessInner<'w, 's, L>,
    commands: Commands<'w, 's>,
}

unsafe impl<'w2, 's2, L: RandomResourceList> SystemParam for RandomAccess<'w2, 's2, L> {
    type State = (
        <RandomAccessInner<'w2, 's2, L> as SystemParam>::State,
        <Commands<'w2, 's2> as SystemParam>::State,
    );

    type Item<'w, 's> = RandomAccess<'w, 's, L>;

    fn init_state(world: &mut World, system_meta: &mut SystemMeta) -> Self::State {
        (
            RandomAccessInner::<L>::init_state(world, system_meta),
            Commands::init_state(world, system_meta),
        )
    }

    fn apply(state: &mut Self::State, system_meta: &SystemMeta, world: &mut World) {
        RandomAccessInner::<L>::apply(&mut state.0, system_meta, world);
        Commands::apply(&mut state.1, system_meta, world);
    }

    // TODO: Do we also have to handle new archetypes?

    unsafe fn get_param<'world, 'state>(
        state: &'state mut Self::State,
        system_meta: &SystemMeta,
        world: UnsafeWorldCell<'world>,
        change_tick: Tick,
    ) -> Self::Item<'world, 'state> {
        RandomAccess {
            inner: RandomAccessInner::get_param(&mut state.0, system_meta, world, change_tick),
            commands: Commands::get_param(&mut state.1, system_meta, world, change_tick),
        }
    }
}

pub struct RandomAccessInner<'w, 's, L: RandomResourceList> {
    world: UnsafeWorldCell<'w>,
    state: &'s L::ParamState,
}

unsafe impl<'w2, 's2, L: RandomResourceList> SystemParam for RandomAccessInner<'w2, 's2, L> {
    type State = L::ParamState;
    type Item<'w, 's> = RandomAccessInner<'w, 's, L>;

    fn init_state(world: &mut World, system_meta: &mut SystemMeta) -> Self::State {
        // Fetch the IDs of each component used in the borrow and ensure that they don't conflict
        // with another parameter's borrow access.
        let state = L::get_param_state(world, system_meta);

        // Adjust the borrow set of this system so that future parameters don't conflict with us.
        L::update_access_sets(&state, world, system_meta);

        state
    }

    #[inline]
    unsafe fn get_param<'w, 's>(
        state: &'s mut Self::State,
        _system_meta: &SystemMeta,
        world: UnsafeWorldCell<'w>,
        _change_tick: Tick,
    ) -> Self::Item<'w, 's> {
        RandomAccessInner {
            state: &*state,
            world,
        }
    }
}

impl<'w, 's, L: RandomResourceList> RandomAccess<'w, 's, L> {
    pub fn provide<R>(&mut self, f: impl FnOnce() -> R) -> R {
        unsafe {
            autoken::absorb::<L::TokensMut, R>(|| {
                let new_snap = L::tls_snapshot_from_world(self.inner.state, self.inner.world);
                let _guard = scopeguard::guard(L::fetch_tls_snapshot(), |snap| {
                    L::apply_tls_snapshot(&snap);
                });
                L::apply_tls_snapshot(&new_snap);

                fn dummy<'a, S: TokenSet>() -> &'a () {
                    autoken::tie!('a => set S);
                    &()
                }

                let _all = dummy::<L::TokensMut>();
                autoken::absorb::<L::Tokens, R>(|| {
                    CommandsCap::provide(&mut self.commands, || {
                        WorldCap::provide(&self.inner.world, f)
                    })
                })
            })
        }
    }
}

// === RandomComponentList === //

pub type RandBorrowsMut<'a, T> = &'a mut Borrows<RandTokensOf<T>>;

pub type RandBorrowsRef<'a, T> = &'a Borrows<RandTokensOf<T>>;

pub type RandTokensOf<T> = (<T as RandomResourceList>::Tokens, Ref<WorldCap>);

pub unsafe trait RandomResourceList {
    /// The set of tokens absorbed by the list.
    type Tokens: TokenSet;

    /// The set of tokens absorbed by the list but each token is promoted to its mutable form.
    type TokensMut: TokenSet;

    /// The state of our [`RandomAccess`] system parameter.
    type ParamState: 'static + Copy + Send + Sync;

    type TlsSnapshot: 'static + Copy;

    /// Fetches the set of [`ComponentId`]s that this component list, ensuring that the existing
    /// system meta doesn't have any conflicting borrows with other systems.
    fn get_param_state(world: &mut World, system_meta: &mut SystemMeta) -> Self::ParamState;

    /// Appends this set's resource set to the system metadata.
    fn update_access_sets(
        state: &Self::ParamState,
        world: &mut World,
        system_meta: &mut SystemMeta,
    );

    /// Fetch a snapshot of all previous arena TLS states.
    fn fetch_tls_snapshot() -> Self::TlsSnapshot;

    /// Compute new snapshot from world resources.
    unsafe fn tls_snapshot_from_world(
        state: &Self::ParamState,
        world: UnsafeWorldCell<'_>,
    ) -> Self::TlsSnapshot;

    /// Applies a snapshot on arena TLS states.
    unsafe fn apply_tls_snapshot(snap: &Self::TlsSnapshot);
}

unsafe impl<T: RandomComponent> RandomResourceList for &'_ T {
    type Tokens = autoken::Ref<RandomComponentToken<T>>;
    type TokensMut = autoken::Mut<RandomComponentToken<T>>;
    type ParamState = ComponentId;
    type TlsSnapshot = *mut RandomArena<T>;

    fn get_param_state(world: &mut World, system_meta: &mut SystemMeta) -> Self::ParamState {
        // TODO: Use an alias-permitting technique
        // let component_id = world.init_resource::<RandomArena<T>>();
        //
        // let combined_access = system_meta.component_access_set.combined_access();
        // assert!(
        //     !combined_access.has_write(component_id),
        //     "error[B0002]: Res<{}> in system {} conflicts with a previous ResMut<{0}> access. Consider removing the duplicate access.",
        //     std::any::type_name::<T>(),
        //     system_meta.name(),
        // );
        //
        // component_id

        <Res<RandomArena<T>> as SystemParam>::init_state(world, system_meta)
    }

    fn update_access_sets(
        &component_id: &Self::ParamState,
        world: &mut World,
        system_meta: &mut SystemMeta,
    ) {
        let _ = (component_id, world, system_meta);

        // TODO: Use an alias-permitting technique
        // system_meta
        //     .component_access_set
        //     .add_unfiltered_read(component_id);
        //
        // let archetype_component_id = world
        //     .get_resource_archetype_component_id(component_id)
        //     .unwrap();
        //
        // system_meta
        //     .archetype_component_access
        //     .add_read(archetype_component_id);
    }

    fn fetch_tls_snapshot() -> Self::TlsSnapshot {
        unsafe { T::tls().get() }
    }

    unsafe fn tls_snapshot_from_world(
        &state: &Self::ParamState,
        world: UnsafeWorldCell<'_>,
    ) -> Self::TlsSnapshot {
        world
            .get_resource_by_id(state)
            .unwrap_or_else(|| {
                panic!(
                    "Random component never registered: {}",
                    std::any::type_name::<T>()
                )
            })
            .as_ptr()
            .cast()
    }

    unsafe fn apply_tls_snapshot(&snap: &Self::TlsSnapshot) {
        unsafe { T::tls().set(snap) }
    }
}

unsafe impl<T: RandomComponent> RandomResourceList for &'_ mut T {
    type Tokens = autoken::Mut<RandomComponentToken<T>>;
    type TokensMut = autoken::Mut<RandomComponentToken<T>>;
    type ParamState = ComponentId;
    type TlsSnapshot = *mut RandomArena<T>;

    fn get_param_state(world: &mut World, system_meta: &mut SystemMeta) -> Self::ParamState {
        // TODO: Use an alias-permitting technique
        // let component_id = world.init_resource::<RandomArena<T>>();
        //
        // let combined_access = system_meta.component_access_set.combined_access();
        // assert!(
        //     !combined_access.has_write(component_id),
        //     "error[B0002]: Res<{}> in system {} conflicts with a previous ResMut<{0}> access. Consider removing the duplicate access.",
        //     std::any::type_name::<T>(),
        //     system_meta.name(),
        // );
        //
        // component_id

        <ResMut<RandomArena<T>> as SystemParam>::init_state(world, system_meta)
    }

    fn update_access_sets(
        &component_id: &Self::ParamState,
        world: &mut World,
        system_meta: &mut SystemMeta,
    ) {
        let _ = (component_id, world, system_meta);

        // TODO: Use an alias-permitting technique
        // system_meta
        //     .component_access_set
        //     .add_unfiltered_read(component_id);
        //
        // let archetype_component_id = world
        //     .get_resource_archetype_component_id(component_id)
        //     .unwrap();
        //
        // system_meta
        //     .archetype_component_access
        //     .add_read(archetype_component_id);
    }

    fn fetch_tls_snapshot() -> Self::TlsSnapshot {
        unsafe { T::tls().get() }
    }

    unsafe fn tls_snapshot_from_world(
        &state: &Self::ParamState,
        world: UnsafeWorldCell<'_>,
    ) -> Self::TlsSnapshot {
        world
            .get_resource_by_id(state)
            .unwrap_or_else(|| {
                panic!(
                    "Random component never registered: {}",
                    std::any::type_name::<T>()
                )
            })
            .as_ptr()
            .cast()
    }

    unsafe fn apply_tls_snapshot(&snap: &Self::TlsSnapshot) {
        unsafe { T::tls().set(snap) }
    }
}

pub struct SendsEvent<T>(PhantomData<fn() -> T>);

unsafe impl<T: RandomEvent> RandomResourceList for SendsEvent<T> {
    type Tokens = autoken::Mut<RandomEventToken<T>>;
    type TokensMut = autoken::Mut<RandomEventToken<T>>;
    type ParamState = ComponentId;
    type TlsSnapshot = *mut Events<T>;

    fn get_param_state(world: &mut World, system_meta: &mut SystemMeta) -> Self::ParamState {
        // TODO: Use an alias-permitting technique
        // let component_id = world.init_resource::<RandomArena<T>>();
        //
        // let combined_access = system_meta.component_access_set.combined_access();
        // assert!(
        //     !combined_access.has_write(component_id),
        //     "error[B0002]: Res<{}> in system {} conflicts with a previous ResMut<{0}> access. Consider removing the duplicate access.",
        //     std::any::type_name::<T>(),
        //     system_meta.name(),
        // );
        //
        // component_id

        <ResMut<Events<T>> as SystemParam>::init_state(world, system_meta)
    }

    fn update_access_sets(
        &component_id: &Self::ParamState,
        world: &mut World,
        system_meta: &mut SystemMeta,
    ) {
        let _ = (component_id, world, system_meta);

        // TODO: Use an alias-permitting technique
        // system_meta
        //     .component_access_set
        //     .add_unfiltered_read(component_id);
        //
        // let archetype_component_id = world
        //     .get_resource_archetype_component_id(component_id)
        //     .unwrap();
        //
        // system_meta
        //     .archetype_component_access
        //     .add_read(archetype_component_id);
    }

    fn fetch_tls_snapshot() -> Self::TlsSnapshot {
        unsafe { T::tls().get() }
    }

    unsafe fn tls_snapshot_from_world(
        &state: &Self::ParamState,
        world: UnsafeWorldCell<'_>,
    ) -> Self::TlsSnapshot {
        world
            .get_resource_by_id(state)
            .unwrap_or_else(|| panic!("Event never registered: {}", std::any::type_name::<T>()))
            .as_ptr()
            .cast()
    }

    unsafe fn apply_tls_snapshot(&snap: &Self::TlsSnapshot) {
        unsafe { T::tls().set(snap) }
    }
}

unsafe impl RandomResourceList for () {
    type Tokens = ();
    type TokensMut = ();
    type ParamState = ();
    type TlsSnapshot = ();

    fn get_param_state(_world: &mut World, _system_meta: &mut SystemMeta) -> Self::ParamState {}

    fn update_access_sets(
        _state: &Self::ParamState,
        _world: &mut World,
        _system_meta: &mut SystemMeta,
    ) {
    }

    fn fetch_tls_snapshot() -> Self::TlsSnapshot {}

    unsafe fn tls_snapshot_from_world(
        _state: &Self::ParamState,
        _world: UnsafeWorldCell<'_>,
    ) -> Self::TlsSnapshot {
    }

    unsafe fn apply_tls_snapshot(_snap: &Self::TlsSnapshot) {}
}

macro_rules! impl_random_resource_list {
    () => {};
    ($first:ident $($rest:ident)*) => {
        unsafe impl<$first: RandomResourceList $(, $rest: RandomResourceList)*> RandomResourceList for ($first, $($rest,)*) {
            type Tokens = ($first::Tokens, $($rest::Tokens,)*);
            type TokensMut = ($first::TokensMut, $($rest::TokensMut,)*);
            type ParamState = ($first::ParamState, $($rest::ParamState,)*);
            type TlsSnapshot = ($first::TlsSnapshot, $($rest::TlsSnapshot,)*);

            fn get_param_state(world: &mut World, system_meta: &mut SystemMeta) -> Self::ParamState {
                ($first::get_param_state(world, system_meta), $($rest::get_param_state(world, system_meta),)*)
            }

            #[allow(non_snake_case)]
            fn update_access_sets(
                ($first, $($rest,)*): &Self::ParamState,
                world: &mut World,
                system_meta: &mut SystemMeta,
            ) {
                $first::update_access_sets($first, world, system_meta);
                $($rest::update_access_sets($rest, world, system_meta);)*
            }

            fn fetch_tls_snapshot() -> Self::TlsSnapshot {
                ($first::fetch_tls_snapshot(), $($rest::fetch_tls_snapshot(),)*)
            }

            #[allow(non_snake_case)]
            unsafe fn tls_snapshot_from_world(($first, $($rest,)*): &Self::ParamState, world: UnsafeWorldCell<'_>,) -> Self::TlsSnapshot {
                ($first::tls_snapshot_from_world($first, world), $($rest::tls_snapshot_from_world($rest, world),)*)
            }

            #[allow(non_snake_case)]
            unsafe fn apply_tls_snapshot(($first, $($rest,)*): &Self::TlsSnapshot) {
                $first::apply_tls_snapshot($first);
                $($rest::apply_tls_snapshot($rest);)*
            }
        }

        impl_random_resource_list!($($rest)*);
    };
}

impl_random_resource_list!(T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11 T12);

// === RandomComponent === //

pub struct RandomComponentToken<T> {
    _ty: PhantomData<fn() -> T>,
}

pub unsafe trait RandomComponent: 'static + Sized + Send + Sync {
    unsafe fn tls() -> &'static LocalKey<Cell<*mut RandomArena<Self>>>;

    fn arena<'a>() -> &'a RandomArena<Self> {
        autoken::tie!('a => ref RandomComponentToken<Self>);
        autoken::tie!('a => ref WorldCap);

        unsafe { &*Self::tls().get() }
    }

    fn arena_mut<'a>() -> &'a mut RandomArena<Self> {
        autoken::tie!('a => mut RandomComponentToken<Self>);
        autoken::tie!('a => ref WorldCap);

        unsafe { &mut *Self::tls().get() }
    }
}

#[doc(hidden)]
pub mod random_component_internals {
    pub use {
        super::{RandomArena, RandomComponent},
        std::{cell::Cell, ptr::null_mut, thread::LocalKey, thread_local},
    };
}

#[macro_export]
macro_rules! random_component {
    ($($ty:ty),*$(,)?) => {$(
        unsafe impl $crate::random_component_internals::RandomComponent for $ty {
            unsafe fn tls() -> &'static $crate::random_component_internals::LocalKey<
                $crate::random_component_internals::Cell<
                    *mut $crate::random_component_internals::RandomArena<Self>,
                >>
            {
                $crate::random_component_internals::thread_local! {
                    static TLS: $crate::random_component_internals::Cell<
                        *mut $crate::random_component_internals::RandomArena<$ty>,
                    > = const {
                        $crate::random_component_internals::Cell::new(
                            $crate::random_component_internals::null_mut(),
                        )
                    };
                }

                &TLS
            }
        }
    )*};
}

// === RandomEvent === //

pub struct RandomEventToken<T> {
    _ty: PhantomData<fn() -> T>,
}

pub unsafe trait RandomEvent: 'static + Sized + Send + Sync + Event {
    unsafe fn tls() -> &'static LocalKey<Cell<*mut Events<Self>>>;

    fn events<'a>() -> &'a Events<Self> {
        autoken::tie!('a => ref RandomEventToken<Self>);
        unsafe { &*Self::tls().get() }
    }

    fn events_mut<'a>() -> &'a mut Events<Self> {
        autoken::tie!('a => mut RandomEventToken<Self>);
        unsafe { &mut *Self::tls().get() }
    }
}

#[doc(hidden)]
pub mod random_event_internals {
    pub use {
        super::RandomEvent,
        bevy_ecs::event::Events,
        std::{cell::Cell, ptr::null_mut, thread::LocalKey, thread_local},
    };
}

#[macro_export]
macro_rules! random_event {
    ($($ty:ty),*$(,)?) => {$(
        unsafe impl $crate::random_event_internals::RandomEvent for $ty {
            unsafe fn tls() -> &'static $crate::random_event_internals::LocalKey<
                $crate::random_event_internals::Cell<
                    *mut $crate::random_event_internals::Events<Self>,
                >>
            {
                $crate::random_event_internals::thread_local! {
                    static TLS: $crate::random_event_internals::Cell<
                        *mut $crate::random_event_internals::Events<$ty>,
                    > = const {
                        $crate::random_event_internals::Cell::new(
                            $crate::random_event_internals::null_mut(),
                        )
                    };
                }

                &TLS
            }
        }
    )*};
}

// === Obj === //

#[repr(transparent)]
pub struct Obj<T> {
    _ty: PhantomData<fn() -> T>,
    index: Index,
}

impl<T> fmt::Debug for Obj<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Obj")
            .field(&self.index.into_raw_parts().0)
            .field(&self.index.into_raw_parts().1)
            .finish()
    }
}

impl<T> Copy for Obj<T> {}

impl<T> Clone for Obj<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Eq for Obj<T> {}

impl<T> PartialEq for Obj<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
    }
}

impl<T> hash::Hash for Obj<T> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.index.hash(state);
    }
}

impl<T: RandomComponent> Obj<T> {
    fn new(owner: Entity, value: T) -> Self {
        let arena = T::arena_mut();
        match arena.map.entry(owner) {
            hash_map::Entry::Occupied(entry) => {
                let obj = *entry.into_mut();
                arena.arena[obj.index] = (owner, value);
                obj
            }
            hash_map::Entry::Vacant(entry) => {
                let obj = Self::from_index(arena.arena.insert((owner, value)));
                cap!(mut CommandsCap => v in {
                    v.entity(owner).insert(ObjOwner(obj));
                });
                entry.insert(obj);
                obj
            }
        }
    }

    pub fn entity(self) -> Entity {
        T::arena().arena[self.index].0
    }

    pub fn is_alive(self) -> bool {
        T::arena().arena.contains(self.index)
    }

    #[allow(clippy::should_implement_trait)]
    pub fn deref<'a>(self) -> &'a T {
        autoken::tie!('a => ref RandomComponentToken<T>);
        autoken::tie!('a => ref WorldCap);

        &T::arena().arena[self.index].1
    }

    #[allow(clippy::should_implement_trait)]
    pub fn deref_mut<'a>(self) -> &'a mut T {
        autoken::tie!('a => mut RandomComponentToken<T>);
        autoken::tie!('a => ref WorldCap);

        &mut T::arena_mut().arena[self.index].1
    }
}

impl<T> Obj<T> {
    pub fn from_index(index: Index) -> Self {
        Self {
            _ty: PhantomData,
            index,
        }
    }

    pub fn index(me: Self) -> Index {
        me.index
    }
}

impl<T: RandomComponent> Deref for Obj<T> {
    type Target = T;

    fn deref<'a>(&'a self) -> &'a Self::Target {
        autoken::tie!(unsafe 'a => ref RandomComponentToken<T>);
        autoken::tie!(unsafe 'a => ref WorldCap);

        (*self).deref()
    }
}

impl<T: RandomComponent> DerefMut for Obj<T> {
    fn deref_mut<'a>(&'a mut self) -> &'a mut Self::Target {
        autoken::tie!(unsafe 'a => mut RandomComponentToken<T>);
        autoken::tie!(unsafe 'a => ref WorldCap);

        (*self).deref_mut()
    }
}

pub trait RandomEntityExt {
    fn insert<T: RandomComponent>(self, value: T) -> Obj<T>;

    fn remove<T: RandomComponent>(self);

    fn has<T: RandomComponent>(self) -> bool;

    fn try_get<T: RandomComponent>(self) -> Option<Obj<T>>;

    fn get<T: RandomComponent>(self) -> Obj<T>;
}

impl RandomEntityExt for Entity {
    fn insert<T: RandomComponent>(self, value: T) -> Obj<T> {
        Obj::new(self, value)
    }

    fn remove<T: RandomComponent>(self) {
        cap!(mut CommandsCap => v in {
            v.entity(self).remove::<ObjOwner<T>>();
        });
    }

    fn has<T: RandomComponent>(self) -> bool {
        T::arena().map.contains_key(&self)
    }

    fn try_get<T: RandomComponent>(self) -> Option<Obj<T>> {
        T::arena().map.get(&self).copied()
    }

    fn get<T: RandomComponent>(self) -> Obj<T> {
        self.try_get::<T>().unwrap()
    }
}

// === System Link === //

#[derive(Component)]
pub struct ObjOwner<T>(pub Obj<T>);

impl<T> fmt::Debug for ObjOwner<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ObjOwner").field(&self.0).finish()
    }
}

impl<T> Copy for ObjOwner<T> {}

impl<T> Clone for ObjOwner<T> {
    fn clone(&self) -> Self {
        *self
    }
}

pub trait RandomAppExt {
    fn add_random_component<T: RandomComponent>(&mut self);
}

impl RandomAppExt for App {
    fn add_random_component<T: RandomComponent>(&mut self) {
        self.init_resource::<RandomArena<T>>();
        self.add_systems(Last, make_unlinker_system::<T>());
    }
}

pub trait RandomWorldExt {
    fn use_random<L, R>(&mut self, f: impl FnOnce() -> R) -> R
    where
        L: 'static + RandomResourceList;
}

impl RandomWorldExt for World {
    fn use_random<L: 'static + RandomResourceList, R>(&mut self, f: impl FnOnce() -> R) -> R {
        #[allow(clippy::type_complexity)]
        struct Smuggle<L: RandomResourceList>(NonNull<dyn FnMut(RandomAccess<L>)>);

        unsafe impl<L: RandomResourceList> Send for Smuggle<L> {}

        unsafe impl<L: RandomResourceList> Sync for Smuggle<L> {}

        fn sys_use_random<L: RandomResourceList>(
            In(Smuggle(mut f)): In<Smuggle<L>>,
            rand: RandomAccess<L>,
        ) {
            unsafe {
                f.as_mut()(rand);
            }
        }

        let mut f = Some(f);
        let mut result = None;

        let mut f = |mut rand: RandomAccess<L>| {
            rand.provide(|| {
                result = Some(cap!(ref WorldCap => world in {
                    let mut world = *world;

                    WorldCap::provide(&mut world, f.take().unwrap())
                }));
            });
        };

        #[allow(clippy::unnecessary_cast)]
        self.run_system_once_with(
            Smuggle(unsafe {
                NonNull::new_unchecked(
                    &mut f as *mut (dyn FnMut(RandomAccess<L>) + '_)
                        as *mut dyn FnMut(RandomAccess<L>),
                )
            }),
            sys_use_random::<L>,
        );

        result.unwrap()
    }
}

pub fn make_unlinker_system<T: RandomComponent>(
) -> impl 'static + Send + Sync + Fn(RandomAccess<&mut T>, RemovedComponents<ObjOwner<T>>) {
    |mut rand, mut removed| {
        rand.provide(|| {
            let arena = T::arena_mut();

            for removed in removed.read() {
                if let Some(obj) = arena.map.remove(&removed) {
                    arena.arena.remove(obj.index);
                }
            }
        });
    }
}

pub fn spawn_entity(bundle: impl Bundle) -> Entity {
    cap!(mut CommandsCap => v in {
        v.spawn(bundle).id()
    })
}

pub fn despawn_entity(entity: Entity) {
    cap!(mut CommandsCap => v in {
        v.entity(entity).despawn()
    })
}

pub fn send_event<E: RandomEvent>(event: E) {
    E::events_mut().send(event);
}

pub fn world_mut<'a>() -> &'a mut World {
    autoken::tie!('a => mut WorldCap);

    cap!(mut WorldCap => world in unsafe { world.world_mut() })
}
