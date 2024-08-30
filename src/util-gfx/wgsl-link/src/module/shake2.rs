// FIXME: The old shaker does not properly push the arena values in toposorted order.
//  Rewrite it and fix it!

use std::{
    any::Any,
    cell::{RefCell, RefMut},
};

use crucible_utils::newtypes::Index;

use super::merge::RawNagaHandle;

// === Core === //

// ShakeContext
pub struct ShakeContext<D>(RefCell<D>);

impl<D> ShakeContext<D> {
    pub fn new(value: D) -> Self {
        Self(RefCell::new(value))
    }

    pub fn finish(self) -> D {
        self.0.into_inner()
    }
}

// ArenaShaker
#[derive(Copy, Clone)]
pub struct ArenaShaker<'a> {
    handler: &'a dyn Fn(ArenaShakerReq<'_>),
}

impl<'a> ArenaShaker<'a> {
    pub fn new(handler: &'a dyn Fn(ArenaShakerReq<'_>)) -> Self {
        Self { handler }
    }

    pub fn include<T: 'static>(self, handle: naga::Handle<T>) -> naga::Handle<T> {
        let mut input_output = (handle, RawNagaHandle::from_usize(0).as_typed::<T>());
        (self.handler)(ArenaShakerReq {
            shaker: self,
            input_output: &mut input_output,
        });

        input_output.1
    }
}

// ArenaShakerReq
pub struct ArenaShakerReq<'a> {
    shaker: ArenaShaker<'a>,
    input_output: &'a mut dyn Any,
}

impl<'a> ArenaShakerReq<'a> {
    pub fn process<'b, D, T: 'static>(
        self,
        cx: &'b ShakeContext<D>,
        f: impl FnOnce(ArenaShakerReqBound<'a, 'b, D>, naga::Handle<T>) -> naga::Handle<T>,
    ) -> Self {
        if let Some((input, output)) = self
            .input_output
            .downcast_mut::<(naga::Handle<T>, naga::Handle<T>)>()
        {
            *output = f(
                ArenaShakerReqBound {
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

// ArenaShakerReqBound
pub struct ArenaShakerReqBound<'a, 'b, D> {
    shaker: ArenaShaker<'a>,
    context: &'b ShakeContext<D>,
    context_bound: Option<RefMut<'b, D>>,
}

impl<'a, 'b, D> ArenaShakerReqBound<'a, 'b, D> {
    pub fn cx(&self) -> &D {
        self.context_bound.as_ref().unwrap()
    }

    pub fn cx_mut(&mut self) -> &mut D {
        self.context_bound.as_mut().unwrap()
    }

    pub fn include<T: 'static>(&mut self, handle: naga::Handle<T>) -> naga::Handle<T> {
        self.context_bound = None;
        let handle = self.shaker.include(handle);
        self.context_bound = Some(self.context.0.borrow_mut());
        handle
    }
}
