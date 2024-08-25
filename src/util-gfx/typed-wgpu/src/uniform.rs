use crucible_utils::{macros::impl_tuples, newtypes::transparent};
use derive_where::derive_where;
use std::{any::type_name, fmt, hash::Hash, marker::PhantomData, num::NonZeroU32};

use crate::{
    buffer::{DynamicOffset, GpuStruct},
    pipeline::PipelineSet,
    util::SlotAssigner,
};

// === PipelineLayout === //

#[derive_where(Debug)]
#[transparent(raw, pub wrap)]
#[repr(transparent)]
pub struct PipelineLayout<T>
where
    T: PipelineSet,
{
    pub _ty: PhantomData<fn(T)>,
    pub raw: wgpu::PipelineLayout,
}

// === BindGroup === //

#[derive_where(Debug)]
#[transparent(raw, pub wrap)]
#[repr(transparent)]
pub struct BindGroupLayout<T>
where
    T: BindGroup,
{
    pub _ty: PhantomData<fn(T)>,
    pub raw: wgpu::BindGroupLayout,
}

impl<T: BindGroup> BindGroupLayout<T> {
    pub const fn wrap(raw: wgpu::BindGroupLayout) -> Self {
        Self {
            _ty: PhantomData,
            raw,
        }
    }
}

#[derive_where(Debug)]
#[transparent(raw, pub wrap)]
#[repr(transparent)]
pub struct BindGroupInstance<T>
where
    T: BindGroup,
{
    pub _ty: PhantomData<fn(T)>,
    pub raw: wgpu::BindGroup,
}

impl<T: BindGroup> BindGroupInstance<T> {
    pub const fn wrap(raw: wgpu::BindGroup) -> Self {
        Self {
            _ty: PhantomData,
            raw,
        }
    }
}

pub trait BindGroup: Sized {
    type Config: 'static + Hash + Eq + Clone;
    type DynamicOffsets: DynamicOffsetSet;

    fn layout(builder: &mut impl BindGroupBuilder<Self>, config: &Self::Config);

    fn create_layout(device: &wgpu::Device, config: &Self::Config) -> BindGroupLayout<Self> {
        let mut builder = BindGroupLayoutBuilder::default();
        <BindGroupLayoutBuilder as BindGroupBuilder<Self>>::with_label(
            &mut builder,
            type_name::<Self>(),
        );
        Self::layout(&mut builder, config);

        BindGroupLayout::wrap(builder.finish(device))
    }

    fn create_instance(
        &self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        config: &Self::Config,
    ) -> BindGroupInstance<Self> {
        let mut builder = BindGroupInstanceBuilder::new(self);
        Self::layout(&mut builder, config);

        BindGroupInstance::wrap(builder.finish(device, layout))
    }
}

pub trait BindGroupBuilder<T: ?Sized>: fmt::Debug {
    fn with_label(&mut self, label: &str) -> &mut Self;

    fn with_binding(&mut self, loc: u32) -> &mut Self;

    fn with_uniform_buffer<F>(
        &mut self,
        visibility: wgpu::ShaderStages,
        has_dynamic_offset: bool,
        getter: F,
    ) -> &mut Self
    where
        F: FnOnce(&T) -> wgpu::BufferBinding;

    fn with_uniform_buffer_array<F>(
        &mut self,
        visibility: wgpu::ShaderStages,
        has_dynamic_offset: bool,
        count: NonZeroU32,
        getter: F,
    ) -> &mut Self
    where
        F: FnOnce(&T) -> &[wgpu::BufferBinding];

    fn with_storage_buffer<F>(
        &mut self,
        visibility: wgpu::ShaderStages,
        read_only: bool,
        has_dynamic_offset: bool,
        getter: F,
    ) -> &mut Self
    where
        F: FnOnce(&T) -> wgpu::BufferBinding;

    fn with_storage_buffer_array<F>(
        &mut self,
        visibility: wgpu::ShaderStages,
        read_only: bool,
        has_dynamic_offset: bool,
        count: NonZeroU32,
        getter: F,
    ) -> &mut Self
    where
        F: FnOnce(&T) -> &[wgpu::BufferBinding];

    fn with_sampler<F>(
        &mut self,
        visibility: wgpu::ShaderStages,
        ty: wgpu::SamplerBindingType,
        getter: F,
    ) -> &mut Self
    where
        F: FnOnce(&T) -> &wgpu::Sampler;

    fn with_sampler_array<F>(
        &mut self,
        visibility: wgpu::ShaderStages,
        ty: wgpu::SamplerBindingType,
        count: NonZeroU32,
        getter: F,
    ) -> &mut Self
    where
        F: FnOnce(&T) -> &[&wgpu::Sampler];

    fn with_texture<F>(
        &mut self,
        visibility: wgpu::ShaderStages,
        sample_type: wgpu::TextureSampleType,
        view_dimension: wgpu::TextureViewDimension,
        multisampled: bool,
        getter: F,
    ) -> &mut Self
    where
        F: FnOnce(&T) -> &wgpu::TextureView;

    fn with_texture_array<F>(
        &mut self,
        visibility: wgpu::ShaderStages,
        sample_type: wgpu::TextureSampleType,
        view_dimension: wgpu::TextureViewDimension,
        multisampled: bool,
        count: NonZeroU32,
        getter: F,
    ) -> &mut Self
    where
        F: FnOnce(&T) -> &[&wgpu::TextureView];

    fn with_storage_texture<F>(
        &mut self,
        visibility: wgpu::ShaderStages,
        access: wgpu::StorageTextureAccess,
        format: wgpu::TextureFormat,
        view_dimension: wgpu::TextureViewDimension,
        getter: F,
    ) -> &mut Self
    where
        F: FnOnce(&T) -> &wgpu::TextureView;

    fn with_storage_texture_array<F>(
        &mut self,
        visibility: wgpu::ShaderStages,
        access: wgpu::StorageTextureAccess,
        format: wgpu::TextureFormat,
        view_dimension: wgpu::TextureViewDimension,
        count: NonZeroU32,
        getter: F,
    ) -> &mut Self
    where
        F: FnOnce(&T) -> &[&wgpu::TextureView];
}

#[derive(Debug, Default)]
struct BindGroupLayoutBuilder {
    label: Option<String>,
    binding: SlotAssigner,
    entries: Vec<wgpu::BindGroupLayoutEntry>,
}

impl<T: ?Sized> BindGroupBuilder<T> for BindGroupLayoutBuilder {
    fn with_label(&mut self, label: &str) -> &mut Self {
        self.label = Some(label.to_string());
        self
    }

    fn with_binding(&mut self, loc: u32) -> &mut Self {
        self.binding.jump_to(loc);
        self
    }

    fn with_uniform_buffer<F>(
        &mut self,
        visibility: wgpu::ShaderStages,
        has_dynamic_offset: bool,
        _getter: F,
    ) -> &mut Self
    where
        F: FnOnce(&T) -> wgpu::BufferBinding,
    {
        let binding = self.binding.next();
        self.entries.push(wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset,
                min_binding_size: None,
            },
            count: None,
        });
        self
    }

    fn with_uniform_buffer_array<F>(
        &mut self,
        visibility: wgpu::ShaderStages,
        has_dynamic_offset: bool,
        count: NonZeroU32,
        _getter: F,
    ) -> &mut Self
    where
        F: FnOnce(&T) -> &[wgpu::BufferBinding],
    {
        let binding = self.binding.next();
        self.entries.push(wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset,
                min_binding_size: None,
            },
            count: Some(count),
        });
        self
    }

    fn with_storage_buffer<F>(
        &mut self,
        visibility: wgpu::ShaderStages,
        read_only: bool,
        has_dynamic_offset: bool,
        _getter: F,
    ) -> &mut Self
    where
        F: FnOnce(&T) -> wgpu::BufferBinding,
    {
        let binding = self.binding.next();
        self.entries.push(wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only },
                has_dynamic_offset,
                min_binding_size: None,
            },
            count: None,
        });
        self
    }

    fn with_storage_buffer_array<F>(
        &mut self,
        visibility: wgpu::ShaderStages,
        read_only: bool,
        has_dynamic_offset: bool,
        count: NonZeroU32,
        _getter: F,
    ) -> &mut Self
    where
        F: FnOnce(&T) -> &[wgpu::BufferBinding],
    {
        let binding = self.binding.next();
        self.entries.push(wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only },
                has_dynamic_offset,
                min_binding_size: None,
            },
            count: Some(count),
        });
        self
    }

    fn with_sampler<F>(
        &mut self,
        visibility: wgpu::ShaderStages,
        ty: wgpu::SamplerBindingType,
        _getter: F,
    ) -> &mut Self
    where
        F: FnOnce(&T) -> &wgpu::Sampler,
    {
        let binding = self.binding.next();
        self.entries.push(wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgpu::BindingType::Sampler(ty),
            count: None,
        });
        self
    }

    fn with_sampler_array<F>(
        &mut self,
        visibility: wgpu::ShaderStages,
        ty: wgpu::SamplerBindingType,
        count: NonZeroU32,
        _getter: F,
    ) -> &mut Self
    where
        F: FnOnce(&T) -> &[&wgpu::Sampler],
    {
        let binding = self.binding.next();
        self.entries.push(wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgpu::BindingType::Sampler(ty),
            count: Some(count),
        });
        self
    }

    fn with_texture<F>(
        &mut self,
        visibility: wgpu::ShaderStages,
        sample_type: wgpu::TextureSampleType,
        view_dimension: wgpu::TextureViewDimension,
        multisampled: bool,
        _getter: F,
    ) -> &mut Self
    where
        F: FnOnce(&T) -> &wgpu::TextureView,
    {
        let binding = self.binding.next();
        self.entries.push(wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgpu::BindingType::Texture {
                sample_type,
                view_dimension,
                multisampled,
            },
            count: None,
        });
        self
    }

    fn with_texture_array<F>(
        &mut self,
        visibility: wgpu::ShaderStages,
        sample_type: wgpu::TextureSampleType,
        view_dimension: wgpu::TextureViewDimension,
        multisampled: bool,
        count: NonZeroU32,
        _getter: F,
    ) -> &mut Self
    where
        F: FnOnce(&T) -> &[&wgpu::TextureView],
    {
        let binding = self.binding.next();
        self.entries.push(wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgpu::BindingType::Texture {
                sample_type,
                view_dimension,
                multisampled,
            },
            count: Some(count),
        });
        self
    }

    fn with_storage_texture<F>(
        &mut self,
        visibility: wgpu::ShaderStages,
        access: wgpu::StorageTextureAccess,
        format: wgpu::TextureFormat,
        view_dimension: wgpu::TextureViewDimension,
        _getter: F,
    ) -> &mut Self
    where
        F: FnOnce(&T) -> &wgpu::TextureView,
    {
        let binding = self.binding.next();
        self.entries.push(wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgpu::BindingType::StorageTexture {
                access,
                format,
                view_dimension,
            },
            count: None,
        });
        self
    }

    fn with_storage_texture_array<F>(
        &mut self,
        visibility: wgpu::ShaderStages,
        access: wgpu::StorageTextureAccess,
        format: wgpu::TextureFormat,
        view_dimension: wgpu::TextureViewDimension,
        count: NonZeroU32,
        _getter: F,
    ) -> &mut Self
    where
        F: FnOnce(&T) -> &[&wgpu::TextureView],
    {
        let binding = self.binding.next();
        self.entries.push(wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgpu::BindingType::StorageTexture {
                access,
                format,
                view_dimension,
            },
            count: Some(count),
        });
        self
    }
}

impl BindGroupLayoutBuilder {
    pub fn finish(self, device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: self.label.as_deref(),
            entries: &self.entries,
        })
    }
}

#[derive_where(Debug)]
struct BindGroupInstanceBuilder<'a, T: ?Sized> {
    #[derive_where(skip)]
    me: &'a T,
    binding: SlotAssigner,
    entries: Vec<wgpu::BindGroupEntry<'a>>,
}

impl<'a, T: ?Sized> BindGroupInstanceBuilder<'a, T> {
    pub fn new(me: &'a T) -> Self {
        Self {
            me,
            binding: SlotAssigner::default(),
            entries: Vec::default(),
        }
    }
}

impl<'a, T: ?Sized> BindGroupBuilder<T> for BindGroupInstanceBuilder<'a, T> {
    fn with_label(&mut self, _label: &str) -> &mut Self {
        self
    }

    fn with_binding(&mut self, loc: u32) -> &mut Self {
        self.binding.jump_to(loc);
        self
    }

    fn with_uniform_buffer<F>(
        &mut self,
        _visibility: wgpu::ShaderStages,
        _has_dynamic_offset: bool,
        getter: F,
    ) -> &mut Self
    where
        F: FnOnce(&T) -> wgpu::BufferBinding,
    {
        self.entries.push(wgpu::BindGroupEntry {
            binding: self.binding.next(),
            resource: wgpu::BindingResource::Buffer(getter(self.me)),
        });
        self
    }

    fn with_uniform_buffer_array<F>(
        &mut self,
        _visibility: wgpu::ShaderStages,
        _has_dynamic_offset: bool,
        _count: NonZeroU32,
        getter: F,
    ) -> &mut Self
    where
        F: FnOnce(&T) -> &[wgpu::BufferBinding],
    {
        self.entries.push(wgpu::BindGroupEntry {
            binding: self.binding.next(),
            resource: wgpu::BindingResource::BufferArray(getter(self.me)),
        });
        self
    }

    fn with_storage_buffer<F>(
        &mut self,
        _visibility: wgpu::ShaderStages,
        _read_only: bool,
        _has_dynamic_offset: bool,
        getter: F,
    ) -> &mut Self
    where
        F: FnOnce(&T) -> wgpu::BufferBinding,
    {
        self.entries.push(wgpu::BindGroupEntry {
            binding: self.binding.next(),
            resource: wgpu::BindingResource::Buffer(getter(self.me)),
        });
        self
    }

    fn with_storage_buffer_array<F>(
        &mut self,
        _visibility: wgpu::ShaderStages,
        _read_only: bool,
        _has_dynamic_offset: bool,
        _count: NonZeroU32,
        getter: F,
    ) -> &mut Self
    where
        F: FnOnce(&T) -> &[wgpu::BufferBinding],
    {
        self.entries.push(wgpu::BindGroupEntry {
            binding: self.binding.next(),
            resource: wgpu::BindingResource::BufferArray(getter(self.me)),
        });
        self
    }

    fn with_sampler<F>(
        &mut self,
        _visibility: wgpu::ShaderStages,
        _ty: wgpu::SamplerBindingType,
        getter: F,
    ) -> &mut Self
    where
        F: FnOnce(&T) -> &wgpu::Sampler,
    {
        self.entries.push(wgpu::BindGroupEntry {
            binding: self.binding.next(),
            resource: wgpu::BindingResource::Sampler(getter(self.me)),
        });
        self
    }

    fn with_sampler_array<F>(
        &mut self,
        _visibility: wgpu::ShaderStages,
        _ty: wgpu::SamplerBindingType,
        _count: NonZeroU32,
        getter: F,
    ) -> &mut Self
    where
        F: FnOnce(&T) -> &[&wgpu::Sampler],
    {
        self.entries.push(wgpu::BindGroupEntry {
            binding: self.binding.next(),
            resource: wgpu::BindingResource::SamplerArray(getter(self.me)),
        });
        self
    }

    fn with_texture<F>(
        &mut self,
        _visibility: wgpu::ShaderStages,
        _sample_type: wgpu::TextureSampleType,
        _view_dimension: wgpu::TextureViewDimension,
        _multisampled: bool,
        getter: F,
    ) -> &mut Self
    where
        F: FnOnce(&T) -> &wgpu::TextureView,
    {
        self.entries.push(wgpu::BindGroupEntry {
            binding: self.binding.next(),
            resource: wgpu::BindingResource::TextureView(getter(self.me)),
        });
        self
    }

    fn with_texture_array<F>(
        &mut self,
        _visibility: wgpu::ShaderStages,
        _sample_type: wgpu::TextureSampleType,
        _view_dimension: wgpu::TextureViewDimension,
        _multisampled: bool,
        _count: NonZeroU32,
        getter: F,
    ) -> &mut Self
    where
        F: FnOnce(&T) -> &[&wgpu::TextureView],
    {
        self.entries.push(wgpu::BindGroupEntry {
            binding: self.binding.next(),
            resource: wgpu::BindingResource::TextureViewArray(getter(self.me)),
        });
        self
    }

    fn with_storage_texture<F>(
        &mut self,
        _visibility: wgpu::ShaderStages,
        _access: wgpu::StorageTextureAccess,
        _format: wgpu::TextureFormat,
        _view_dimension: wgpu::TextureViewDimension,
        getter: F,
    ) -> &mut Self
    where
        F: FnOnce(&T) -> &wgpu::TextureView,
    {
        self.entries.push(wgpu::BindGroupEntry {
            binding: self.binding.next(),
            resource: wgpu::BindingResource::TextureView(getter(self.me)),
        });
        self
    }

    fn with_storage_texture_array<F>(
        &mut self,
        _visibility: wgpu::ShaderStages,
        _access: wgpu::StorageTextureAccess,
        _format: wgpu::TextureFormat,
        _view_dimension: wgpu::TextureViewDimension,
        _count: NonZeroU32,
        getter: F,
    ) -> &mut Self
    where
        F: FnOnce(&T) -> &[&wgpu::TextureView],
    {
        self.entries.push(wgpu::BindGroupEntry {
            binding: self.binding.next(),
            resource: wgpu::BindingResource::TextureViewArray(getter(self.me)),
        });
        self
    }
}

impl<'a, T: ?Sized> BindGroupInstanceBuilder<'a, T> {
    pub fn finish(self, device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout,
            entries: &self.entries,
        })
    }
}

// === DynamicOffsetSet === //

pub trait DynamicOffsetSet {
    type OffsetSet<'a>: AsRef<[wgpu::DynamicOffset]>
    where
        Self: 'a;

    fn as_offset_set(&self) -> Self::OffsetSet<'_>;
}

pub type NoDynamicOffsets = [wgpu::DynamicOffset; 0];

impl DynamicOffsetSet for [wgpu::DynamicOffset] {
    type OffsetSet<'a> = &'a [wgpu::DynamicOffset]
    where
        Self: 'a;

    fn as_offset_set(&self) -> Self::OffsetSet<'_> {
        self
    }
}

impl<const N: usize> DynamicOffsetSet for [wgpu::DynamicOffset; N] {
    type OffsetSet<'a> = &'a [wgpu::DynamicOffset; N]
    where
        Self: 'a;

    fn as_offset_set(&self) -> Self::OffsetSet<'_> {
        self
    }
}

macro_rules! impl_dynamic_offset_set {
    ($($name:ident:$field:tt),*) => {
        impl<$($name: GpuStruct),*> DynamicOffsetSet for ($(DynamicOffset<$name>,)*) {
            type OffsetSet<'a> = [wgpu::DynamicOffset; { 0 $(+ { let $name = 1; $name })* }]
            where
                Self: 'a;

            fn as_offset_set(&self) -> Self::OffsetSet<'_> {
                [$(self.$field.raw),*]
            }
        }
    };
}

impl_tuples!(impl_dynamic_offset_set);
