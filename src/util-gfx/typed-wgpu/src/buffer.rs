use std::{marker::PhantomData, mem, ops::RangeBounds};

use bytemuck::Pod;
use crucible_utils::newtypes::transparent;
use derive_where::derive_where;
use wgpu::util::DeviceExt as _;

// === GpuStruct === //

pub trait GpuStruct {
    type Pod: Pod;
}

// === Buffer === //

#[derive_where(Debug)]
#[transparent(raw, pub wrap)]
#[repr(transparent)]
pub struct Buffer<T: GpuStruct> {
    pub _ty: PhantomData<fn(T)>,
    pub raw: wgpu::Buffer,
}

impl<T: GpuStruct> Buffer<T> {
    pub const ELEM_SIZE: wgpu::BufferSize =
        match wgpu::BufferSize::new(mem::size_of::<T::Pod>() as wgpu::BufferAddress) {
            Some(v) => v,
            None => panic!("`ELEM_SIZE` is zero"),
        };

    pub const fn wrap(raw: wgpu::Buffer) -> Self {
        Self {
            _ty: PhantomData,
            raw,
        }
    }

    pub fn create(gfx: &wgpu::Device, desc: &wgpu::BufferDescriptor) -> Self {
        Self::wrap(
            gfx.create_buffer(&wgpu::BufferDescriptor {
                label: desc.label,
                size: desc
                    .size
                    .checked_mul(Self::ELEM_SIZE.get())
                    .expect("buffer too big"),
                usage: desc.usage,
                mapped_at_creation: desc.mapped_at_creation,
            }),
        )
    }

    pub fn create_init(gfx: &wgpu::Device, desc: &BufferInitDescriptor<'_, T::Pod>) -> Self {
        Self::wrap(gfx.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: desc.label,
            contents: bytemuck::cast_slice(desc.contents),
            usage: desc.usage,
        }))
    }

    pub fn write(&self, queue: &wgpu::Queue, offset: wgpu::BufferAddress, data: &[T::Pod]) {
        queue.write_buffer(
            &self.raw,
            offset * Self::ELEM_SIZE.get(),
            bytemuck::cast_slice(data),
        )
    }

    pub fn slice(&self, bounds: impl RangeBounds<wgpu::BufferAddress>) -> BufferSlice<'_, T> {
        BufferSlice::wrap(self.raw.slice((
            bounds.start_bound().map(|&v| v * Self::ELEM_SIZE.get()),
            bounds.end_bound().map(|&v| v * Self::ELEM_SIZE.get()),
        )))
    }

    pub fn as_entire_buffer_binding(&self) -> BufferBinding<'_, T> {
        BufferBinding::wrap(self.raw.as_entire_buffer_binding())
    }

    pub fn size(&self) -> wgpu::BufferAddress {
        assert_eq!(self.raw.size() % Self::ELEM_SIZE.get(), 0);

        self.raw.size() / Self::ELEM_SIZE.get()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BufferInitDescriptor<'a, T> {
    pub label: wgpu::Label<'a>,
    pub contents: &'a [T],
    pub usage: wgpu::BufferUsages,
}

// === BufferSlice === //

#[derive_where(Debug, Copy, Clone)]
#[transparent(raw, pub wrap)]
#[repr(transparent)]
pub struct BufferSlice<'a, T: GpuStruct> {
    pub _ty: PhantomData<fn(T)>,
    pub raw: wgpu::BufferSlice<'a>,
}

impl<'a, T: GpuStruct> BufferSlice<'a, T> {
    pub const fn wrap(raw: wgpu::BufferSlice<'a>) -> Self {
        Self {
            _ty: PhantomData,
            raw,
        }
    }
}

// === BufferBinding === //

#[derive_where(Debug, Clone)]
#[transparent(raw, pub wrap)]
#[repr(transparent)]
pub struct BufferBinding<'a, T: GpuStruct> {
    pub _ty: PhantomData<fn(T)>,
    pub raw: wgpu::BufferBinding<'a>,
}

impl<'a, T: GpuStruct> BufferBinding<'a, T> {
    pub const fn wrap(raw: wgpu::BufferBinding<'a>) -> Self {
        Self {
            _ty: PhantomData,
            raw,
        }
    }
}

// === BufferAddress === //

#[derive_where(Debug, Clone)]
#[transparent(raw, pub wrap)]
#[repr(transparent)]
pub struct BufferAddress<T: GpuStruct> {
    pub _ty: PhantomData<fn(T)>,
    pub raw: wgpu::BufferAddress,
}

impl<T: GpuStruct> BufferAddress<T> {
    pub const fn wrap(raw: wgpu::BufferAddress) -> Self {
        Self {
            _ty: PhantomData,
            raw,
        }
    }

    pub fn as_offset(self) -> DynamicOffset<T> {
        DynamicOffset::wrap(wgpu::DynamicOffset::try_from(self.raw).expect("offset too large"))
    }
}

// === DynamicOffset === //

#[derive_where(Debug, Clone)]
#[transparent(raw, pub wrap)]
#[repr(transparent)]
pub struct DynamicOffset<T: GpuStruct> {
    pub _ty: PhantomData<fn(T)>,
    pub raw: wgpu::DynamicOffset,
}

impl<T: GpuStruct> DynamicOffset<T> {
    pub const fn wrap(raw: wgpu::DynamicOffset) -> Self {
        Self {
            _ty: PhantomData,
            raw,
        }
    }
}
