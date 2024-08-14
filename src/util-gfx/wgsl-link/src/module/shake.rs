use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    hash,
    marker::PhantomData,
};

use crucible_utils::{
    hash::{hashbrown::hash_map, FxHashMap},
    newtypes::Index,
};
use naga::Span;

use super::{fold::Folder, merge::RawNagaHandle};

// === ArenaShakeSession === //

#[derive(Debug, Default)]
pub struct ArenaShakeSession {
    is_dirty: Cell<bool>,
}

impl ArenaShakeSession {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn run(&self, mut f: impl FnMut()) {
        loop {
            self.is_dirty.set(false);
            f();
            if !self.is_dirty.get() {
                break;
            }
        }
    }
}

// === ArenaShaker === //

pub struct ArenaShaker<'a, T, A> {
    sess: &'a ArenaShakeSession,
    src_arena: &'a naga::Arena<T>,
    dst_arena: naga::Arena<T>,
    src_handles_to_add: VecDeque<(naga::Handle<T>, A)>,
    src_to_dst: FxHashMap<naga::Handle<T>, naga::Handle<T>>,
}

impl<'a, T, A> ArenaShaker<'a, T, A> {
    pub fn new(sess: &'a ArenaShakeSession, source: &'a naga::Arena<T>) -> Self {
        Self {
            sess,
            src_arena: source,
            dst_arena: naga::Arena::new(),
            src_handles_to_add: VecDeque::new(),
            src_to_dst: FxHashMap::default(),
        }
    }

    pub fn include(
        &mut self,
        src_handle: naga::Handle<T>,
        meta: impl FnOnce() -> A,
    ) -> naga::Handle<T> {
        let entry = match self.src_to_dst.entry(src_handle) {
            hash_map::Entry::Occupied(entry) => {
                return *entry.get();
            }
            hash_map::Entry::Vacant(entry) => entry,
        };

        let dst_handle =
            RawNagaHandle::from_usize(self.dst_arena.len() + self.src_handles_to_add.len())
                .as_typed();

        self.sess.is_dirty.set(true);
        self.src_handles_to_add.push_back((src_handle, meta()));
        entry.insert(dst_handle);
        dst_handle
    }

    pub fn run(&mut self, mut f: impl FnMut(&mut Self, Span, &T, A) -> (Span, T)) {
        while let Some((src_handle, meta)) = self.src_handles_to_add.pop_front() {
            let src_span = self.src_arena.get_span(src_handle);
            let src_value = &self.src_arena[src_handle];
            let (dst_span, dst_value) = f(self, src_span, src_value, meta);
            self.dst_arena.append(dst_value, dst_span);
        }
    }

    pub fn folder<'r>(
        &'r mut self,
        arg_gen: &'r impl Fn() -> A,
    ) -> ArenaShakerFolder<'r, 'a, T, A> {
        ArenaShakerFolder {
            shaker: RefCell::new(self),
            arg_gen,
        }
    }

    pub fn finish(self) -> naga::Arena<T> {
        self.dst_arena
    }
}

pub struct ArenaShakerFolder<'r, 'a, T, A> {
    shaker: RefCell<&'r mut ArenaShaker<'a, T, A>>,
    arg_gen: &'r dyn Fn() -> A,
}

impl<'r, 'a, T, A> Folder<naga::Handle<T>> for ArenaShakerFolder<'r, 'a, T, A> {
    fn fold(&self, value: naga::Handle<T>) -> naga::Handle<T> {
        self.shaker.borrow_mut().include(value, || (self.arg_gen)())
    }
}

// === UniqueArenaShaker === //

#[allow(clippy::type_complexity)]
pub struct UniqueArenaShaker<'a, T, A, D = ()> {
    _ty: PhantomData<fn(A)>,
    data: D,
    src_arena: &'a naga::UniqueArena<T>,
    dst_arena: naga::UniqueArena<T>,
    src_to_dst: FxHashMap<naga::Handle<T>, naga::Handle<T>>,
    mapper: &'a dyn Fn(&mut Self, Span, &T, A) -> (Span, T),
}

impl<'a, T, A, D> UniqueArenaShaker<'a, T, A, D>
where
    T: hash::Hash + Eq,
{
    pub fn new(
        src_arena: &'a naga::UniqueArena<T>,
        data: D,
        mapper: &'a impl Fn(&mut Self, Span, &T, A) -> (Span, T),
    ) -> Self {
        Self {
            _ty: PhantomData,
            data,
            src_arena,
            dst_arena: naga::UniqueArena::new(),
            src_to_dst: FxHashMap::default(),
            mapper,
        }
    }

    pub fn data(&self) -> &D {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut D {
        &mut self.data
    }

    pub fn include(
        &mut self,
        src_handle: naga::Handle<T>,
        args: impl FnOnce() -> A,
    ) -> naga::Handle<T> {
        if let Some(&existing) = self.src_to_dst.get(&src_handle) {
            return existing;
        }

        let src_span = self.src_arena.get_span(src_handle);
        let src_value = &self.src_arena[src_handle];
        let (dst_span, dst_value) = (self.mapper)(self, src_span, src_value, args());
        let dst_handle = self.dst_arena.insert(dst_value, dst_span);
        self.src_to_dst.insert(src_handle, dst_handle);
        dst_handle
    }

    pub fn folder<'r>(
        &'r mut self,
        arg_gen: &'r impl Fn() -> A,
    ) -> UniqueArenaShakerFolder<'r, 'a, T, A, D> {
        UniqueArenaShakerFolder {
            shaker: RefCell::new(self),
            arg_gen,
        }
    }

    pub fn finish(self) -> naga::UniqueArena<T> {
        self.dst_arena
    }
}

pub struct UniqueArenaShakerFolder<'r, 'a, T, A, D = ()> {
    shaker: RefCell<&'r mut UniqueArenaShaker<'a, T, A, D>>,
    arg_gen: &'r dyn Fn() -> A,
}

impl<'r, 'a, T, A, D> Folder<naga::Handle<T>> for UniqueArenaShakerFolder<'r, 'a, T, A, D>
where
    T: hash::Hash + Eq,
{
    fn fold(&self, value: naga::Handle<T>) -> naga::Handle<T> {
        self.shaker.borrow_mut().include(value, || (self.arg_gen)())
    }
}
