use std::marker::PhantomData;

use super::allocator::{SmartAllocator, ObjAllocator};

pub type ObjVec<T> = SmartVec<T, ObjAllocator>;

pub struct SmartVec<T, A: SmartAllocator> {
	_ty: PhantomData<T>,
	base: A::Handle,
	len: usize,
	cap: usize,
	alloc: A,
}

impl<T, A: SmartAllocator> SmartVec<T, A> {
    pub fn new() -> Self {
        todo!()
    }
}
