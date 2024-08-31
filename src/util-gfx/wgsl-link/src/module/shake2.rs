// FIXME: The old shaker does not properly push the arena values in toposorted order.
//  Rewrite it and fix it!

use std::{
    any::Any,
    cell::{RefCell, RefMut},
};

use crucible_utils::{
    hash::{hashbrown::hash_map, FxHashMap},
    newtypes::Index,
};

use super::merge::RawNagaHandle;

// === Core === //

// ShakeContext
pub struct ShakeContext<A, D>(RefCell<(A, D)>);

impl<A, D> ShakeContext<A, D> {
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
}

pub trait ArenaShaker: Sized {
    type Handle: 'static;
    type Finished;

    fn finish(self) -> Self::Finished;
}

pub trait ArenaShakerFor<'a, 'b, D, F>: ArenaShaker {
    fn process(
        req: MultiArenaShakerReqBound<'a, 'b, Self, D>,
        handle: naga::Handle<Self::Handle>,
        f: F,
    ) -> naga::Handle<Self::Handle>;
}

// MultiArenaShaker
#[derive(Copy, Clone)]
pub struct MultiArenaShaker<'a> {
    handler: &'a dyn Fn(MultiArenaShakerReq<'_>),
}

impl<'a> MultiArenaShaker<'a> {
    pub fn new(handler: &'a dyn Fn(MultiArenaShakerReq<'_>)) -> Self {
        Self { handler }
    }

    pub fn include<T: 'static>(self, handle: naga::Handle<T>) -> naga::Handle<T> {
        let mut input_output = (handle, RawNagaHandle::from_usize(0).as_typed::<T>());
        (self.handler)(MultiArenaShakerReq {
            shaker: self,
            input_output: &mut input_output,
        });

        input_output.1
    }
}

// MultiArenaShakerReq
pub struct MultiArenaShakerReq<'a> {
    shaker: MultiArenaShaker<'a>,
    input_output: &'a mut dyn Any,
}

impl<'a> MultiArenaShakerReq<'a> {
    pub fn process<'b, A, D, F>(self, cx: &'b ShakeContext<A, D>, f: F) -> Self
    where
        A: ArenaShakerFor<'a, 'b, D, F>,
    {
        self.process_raw(cx, |req, handle| A::process(req, handle, f))
    }

    pub fn process_raw<'b, A, D, T: 'static>(
        self,
        cx: &'b ShakeContext<A, D>,
        f: impl FnOnce(MultiArenaShakerReqBound<'a, 'b, A, D>, naga::Handle<T>) -> naga::Handle<T>,
    ) -> Self {
        if let Some((input, output)) = self
            .input_output
            .downcast_mut::<(naga::Handle<T>, naga::Handle<T>)>()
        {
            *output = f(
                MultiArenaShakerReqBound {
                    shaker: self.shaker,
                    context: cx,
                    context_bound: Some(cx.0.borrow_mut()),
                },
                *input,
            );
        }

        self
    }
}

// MultiArenaShakerReqBound
pub struct MultiArenaShakerReqBound<'a, 'b, A, D> {
    shaker: MultiArenaShaker<'a>,
    context: &'b ShakeContext<A, D>,
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

        let handle = req.arena_mut().dst.append(value, span);
        *req.arena_mut().map.get_mut(&handle).unwrap() = Some(handle);
        handle
    }
}
