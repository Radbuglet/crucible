// FIXME: The old shaker does not properly push the arena values in toposorted order.
//  Rewrite it and fix it!

use std::{
    any::Any,
    cell::{RefCell, RefMut},
    hash,
};

use crucible_utils::hash::{hashbrown::hash_map, FxHashMap};

use super::map::{Map, UpcastCollectionUnit};

// === Core === //

// AnyMultiShaker
pub trait AnyMultiShaker: Sized {
    fn include<T: 'static>(&mut self, handle: naga::Handle<T>) -> naga::Handle<T>;

    fn mapper(&mut self) -> AnyMultiShakerMapper<'_, Self> {
        AnyMultiShakerMapper(RefCell::new(self))
    }
}

pub struct AnyMultiShakerMapper<'a, S>(RefCell<&'a mut S>);

impl<S, T> Map<naga::Handle<T>, ()> for AnyMultiShakerMapper<'_, S>
where
    S: AnyMultiShaker,
    T: 'static,
{
    fn map(&self, value: naga::Handle<T>) -> naga::Handle<T> {
        self.0.borrow_mut().include(value)
    }
}

// ShakerContext
pub struct ShakerContext<A, D>(RefCell<(A, D)>);

impl<A, D> ShakerContext<A, D> {
    pub fn new(arena: A, data: D) -> Self {
        Self(RefCell::new((arena, data)))
    }

    pub fn finish_raw(self) -> (A, D) {
        self.0.into_inner()
    }

    pub fn finish(self) -> A::Finished
    where
        A: ArenaShaker,
    {
        self.finish_raw().0.finish()
    }

    pub fn upcaster(&self) -> UpcastCollectionUnit<naga::Handle<A::Handle>>
    where
        A: ArenaShaker,
    {
        UpcastCollectionUnit::new()
    }
}

pub trait ArenaShaker: Sized {
    type Handle: 'static;
    type Finished;

    fn finish(self) -> Self::Finished;

    fn wrap<D>(self, data: D) -> ShakerContext<Self, D> {
        ShakerContext::new(self, data)
    }
}

pub trait ArenaShakerFor<'a, 'b, D, F>: ArenaShaker {
    fn process(
        req: MultiArenaShakerReqBound<'a, 'b, Self, D>,
        handle: naga::Handle<Self::Handle>,
        f: F,
    ) -> naga::Handle<Self::Handle>;
}

// MultiArenaShaker
pub struct MultiArenaShaker<'a> {
    handler: &'a dyn Fn(MultiArenaShakerReq<'_>),
}

impl<'a> MultiArenaShaker<'a> {
    pub fn new(handler: &'a dyn Fn(MultiArenaShakerReq<'_>)) -> Self {
        Self { handler }
    }

    pub fn include<T: 'static>(&mut self, handle: naga::Handle<T>) -> naga::Handle<T> {
        let mut input_output = (handle, None);
        (self.handler)(MultiArenaShakerReq {
            shaker: MultiArenaShaker {
                handler: &self.handler,
            },
            input_output: &mut input_output,
        });

        input_output.1.expect("no shaker provided")
    }
}

impl AnyMultiShaker for MultiArenaShaker<'_> {
    fn include<T: 'static>(&mut self, handle: naga::Handle<T>) -> naga::Handle<T> {
        self.include(handle)
    }
}

// MultiArenaShakerReq
pub struct MultiArenaShakerReq<'a> {
    shaker: MultiArenaShaker<'a>,
    input_output: &'a mut dyn Any,
}

impl<'a> MultiArenaShakerReq<'a> {
    pub fn process<'b, A, D, F>(self, cx: &'b ShakerContext<A, D>, f: F) -> Self
    where
        A: ArenaShakerFor<'a, 'b, D, F>,
    {
        self.process_raw(cx, |req, handle| A::process(req, handle, f))
    }

    pub fn process_raw<'b, A, D, T: 'static>(
        self,
        cx: &'b ShakerContext<A, D>,
        f: impl FnOnce(MultiArenaShakerReqBound<'a, 'b, A, D>, naga::Handle<T>) -> naga::Handle<T>,
    ) -> Self {
        if let Some((input, output)) = self
            .input_output
            .downcast_mut::<(naga::Handle<T>, Option<naga::Handle<T>>)>()
        {
            *output = Some(f(
                MultiArenaShakerReqBound {
                    shaker: MultiArenaShaker {
                        handler: self.shaker.handler,
                    },
                    context: cx,
                    context_bound: Some(cx.0.borrow_mut()),
                },
                *input,
            ));
        }

        self
    }
}

// MultiArenaShakerReqBound
pub struct MultiArenaShakerReqBound<'a, 'b, A, D> {
    shaker: MultiArenaShaker<'a>,
    context: &'b ShakerContext<A, D>,
    context_bound: Option<RefMut<'b, (A, D)>>,
}

impl<'a, 'b, A, D> MultiArenaShakerReqBound<'a, 'b, A, D> {
    pub fn cx(&self) -> &(A, D) {
        self.context_bound.as_ref().unwrap()
    }

    pub fn cx_mut(&mut self) -> &mut (A, D) {
        self.context_bound.as_mut().unwrap()
    }

    pub fn arena(&self) -> &A {
        &self.cx().0
    }

    pub fn arena_mut(&mut self) -> &mut A {
        &mut self.cx_mut().0
    }

    pub fn data(&self) -> &D {
        &self.cx().1
    }

    pub fn data_mut(&mut self) -> &mut D {
        &mut self.cx_mut().1
    }

    pub fn include<T: 'static>(&mut self, handle: naga::Handle<T>) -> naga::Handle<T> {
        self.context_bound = None;
        let handle = self.shaker.include(handle);
        self.context_bound = Some(self.context.0.borrow_mut());
        handle
    }
}

impl<'a, 'b, A, D> AnyMultiShaker for MultiArenaShakerReqBound<'a, 'b, A, D> {
    fn include<T: 'static>(&mut self, handle: naga::Handle<T>) -> naga::Handle<T> {
        self.include(handle)
    }
}

// === NagaArenaShaker === //

pub struct NagaArenaShaker<'a, T> {
    src: &'a naga::Arena<T>,
    dst: naga::Arena<T>,
    map: FxHashMap<naga::Handle<T>, Option<naga::Handle<T>>>,
}

impl<'a, T> NagaArenaShaker<'a, T> {
    pub fn new(src: &'a naga::Arena<T>) -> Self {
        Self {
            src,
            dst: naga::Arena::new(),
            map: FxHashMap::default(),
        }
    }
}

impl<T: 'static> ArenaShaker for NagaArenaShaker<'_, T> {
    type Handle = T;
    type Finished = naga::Arena<T>;

    fn finish(self) -> Self::Finished {
        self.dst
    }
}

impl<'a, 'b, T, D, F> ArenaShakerFor<'a, 'b, D, F> for NagaArenaShaker<'_, T>
where
    T: 'static,
    F: FnOnce(&mut MultiArenaShakerReqBound<'a, 'b, Self, D>, naga::Span, &T) -> (naga::Span, T),
{
    fn process(
        mut req: MultiArenaShakerReqBound<'a, 'b, Self, D>,
        handle: naga::Handle<Self::Handle>,
        f: F,
    ) -> naga::Handle<Self::Handle> {
        match req.arena_mut().map.entry(handle) {
            hash_map::Entry::Occupied(entry) => {
                return entry.get().expect("handle has a cyclic definition")
            }
            hash_map::Entry::Vacant(entry) => {
                entry.insert(None);
            }
        }

        let src = req.arena().src;
        let span = src.get_span(handle);
        let value = &src[handle];

        let (span, value) = f(&mut req, span, value);

        let new_handle = req.arena_mut().dst.append(value, span);
        *req.arena_mut().map.get_mut(&handle).unwrap() = Some(new_handle);
        new_handle
    }
}

// === NagaUniqueArenaShaker === //

pub struct NagaUniqueArenaShaker<'a, T> {
    src: &'a naga::UniqueArena<T>,
    dst: naga::UniqueArena<T>,
    map: FxHashMap<naga::Handle<T>, Option<naga::Handle<T>>>,
}

impl<'a, T> NagaUniqueArenaShaker<'a, T> {
    pub fn new(src: &'a naga::UniqueArena<T>) -> Self {
        Self {
            src,
            dst: naga::UniqueArena::new(),
            map: FxHashMap::default(),
        }
    }
}

impl<T: 'static> ArenaShaker for NagaUniqueArenaShaker<'_, T> {
    type Handle = T;
    type Finished = naga::UniqueArena<T>;

    fn finish(self) -> Self::Finished {
        self.dst
    }
}

impl<'a, 'b, T, D, F> ArenaShakerFor<'a, 'b, D, F> for NagaUniqueArenaShaker<'_, T>
where
    T: 'static + hash::Hash + Eq,
    F: FnOnce(&mut MultiArenaShakerReqBound<'a, 'b, Self, D>, naga::Span, &T) -> (naga::Span, T),
{
    fn process(
        mut req: MultiArenaShakerReqBound<'a, 'b, Self, D>,
        handle: naga::Handle<Self::Handle>,
        f: F,
    ) -> naga::Handle<Self::Handle> {
        match req.arena_mut().map.entry(handle) {
            hash_map::Entry::Occupied(entry) => {
                return entry.get().expect("handle has a cyclic definition")
            }
            hash_map::Entry::Vacant(entry) => {
                entry.insert(None);
            }
        }

        let src = req.arena().src;
        let span = src.get_span(handle);
        let value = &src[handle];

        let (span, value) = f(&mut req, span, value);

        let new_handle = req.arena_mut().dst.insert(value, span);
        *req.arena_mut().map.get_mut(&handle).unwrap() = Some(new_handle);
        new_handle
    }
}
