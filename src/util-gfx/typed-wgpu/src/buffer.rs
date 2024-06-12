use std::marker::PhantomData;

use crucible_utils::newtypes::transparent;
use derive_where::derive_where;

#[derive_where(Debug)]
#[transparent(raw, pub wrap)]
#[repr(transparent)]
pub struct Buffer<T> {
    pub _ty: PhantomData<fn(T)>,
    pub raw: wgpu::Buffer,
}

impl<T> Buffer<T> {
    pub const fn wrap(raw: wgpu::Buffer) -> Self {
        Self {
            _ty: PhantomData,
            raw,
        }
    }
}

#[derive_where(Debug, Copy, Clone)]
#[transparent(raw, pub wrap)]
#[repr(transparent)]
pub struct BufferSlice<'a, T> {
    pub _ty: PhantomData<fn(T)>,
    pub raw: wgpu::BufferSlice<'a>,
}

impl<'a, T> BufferSlice<'a, T> {
    pub const fn wrap(raw: wgpu::BufferSlice<'a>) -> Self {
        Self {
            _ty: PhantomData,
            raw,
        }
    }
}

#[derive_where(Debug, Clone)]
#[transparent(raw, pub wrap)]
#[repr(transparent)]
pub struct BufferBinding<'a, T> {
    pub _ty: PhantomData<fn(T)>,
    pub raw: wgpu::BufferBinding<'a>,
}

impl<'a, T> BufferBinding<'a, T> {
    pub const fn wrap(raw: wgpu::BufferBinding<'a>) -> Self {
        Self {
            _ty: PhantomData,
            raw,
        }
    }
}
