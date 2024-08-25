use std::{borrow::Cow, marker::PhantomData};

use crucible_utils::{
    macros::impl_tuples,
    newtypes::{enum_index, transparent},
};
use derive_where::derive_where;

use crate::{
    pipeline::{PipelineSet, UntypedPipelineSet},
    util::SlotAssigner,
};

// === VertexBufferSetLayout === //

pub trait VertexBufferLayoutSet<K: PipelineSet> {
    fn layouts(&self) -> Cow<[wgpu::VertexBufferLayout<'_>]>;
}

impl VertexBufferLayoutSet<UntypedPipelineSet> for [wgpu::VertexBufferLayout<'_>] {
    fn layouts(&self) -> Cow<[wgpu::VertexBufferLayout<'_>]> {
        self.into()
    }
}

impl VertexBufferLayoutSet<()> for () {
    fn layouts(&self) -> Cow<[wgpu::VertexBufferLayout<'_>]> {
        vec![].into()
    }
}

macro_rules! impl_vertex_buffer_set {
	($($para:ident:$field:tt),*) => {
		impl<'a, $($para: 'static),*> VertexBufferLayoutSet<($($para,)*)> for ($(&'a VertexBufferLayout<$para>,)*) {
			fn layouts(&self) -> Cow<[wgpu::VertexBufferLayout<'_>]> {
				vec![$(self.$field.raw.as_wgpu()),*].into()
			}
		}

        impl<'a, $($para: 'static),*> VertexBufferLayoutSet<($($para,)*)> for ($(VertexBufferLayout<$para>,)*) {
			fn layouts(&self) -> Cow<[wgpu::VertexBufferLayout<'_>]> {
				vec![$(self.$field.raw.as_wgpu()),*].into()
			}
		}
	};
}

impl_tuples!(impl_vertex_buffer_set; no_unit);

// === VertexBufferLayout === //

#[derive_where(Debug)]
#[transparent(raw, pub wrap)]
#[repr(transparent)]
pub struct VertexBufferLayout<T> {
    pub _ty: PhantomData<fn(T)>,
    pub raw: RawVertexBufferLayout,
}

impl<T> VertexBufferLayout<T> {
    pub const fn wrap(buffer: RawVertexBufferLayout) -> Self {
        Self {
            _ty: PhantomData,
            raw: buffer,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct RawVertexBufferLayout {
    pub stride: wgpu::BufferAddress,
    pub align: wgpu::BufferAddress,
    pub step_mode: wgpu::VertexStepMode,
    pub attributes: Vec<wgpu::VertexAttribute>,
}

impl RawVertexBufferLayout {
    pub fn as_wgpu(&self) -> wgpu::VertexBufferLayout {
        wgpu::VertexBufferLayout {
            array_stride: self.stride,
            step_mode: self.step_mode,
            attributes: &self.attributes,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct VertexBufferLayoutBuilder {
    location: SlotAssigner,
    size: wgpu::BufferAddress,
    align: wgpu::BufferAddress,
    next_offset: wgpu::BufferAddress,
    attributes: Vec<wgpu::VertexAttribute>,
}

// N.B. we only impl this for some arbitrary type `()` so calls to `VertexBufferLayout::builder()`
// can resolve unambiguously.
impl VertexBufferLayout<()> {
    pub fn builder() -> VertexBufferLayoutBuilder {
        VertexBufferLayoutBuilder::new()
    }
}

impl VertexBufferLayoutBuilder {
    const OVERFLOW_ERR: &'static str = "attribute offset overflowed";

    pub fn new() -> Self {
        Self::default()
    }

    // Getters
    pub fn attributes(&self) -> &[wgpu::VertexAttribute] {
        &self.attributes
    }

    pub fn attributes_mut(&mut self) -> &mut Vec<wgpu::VertexAttribute> {
        &mut self.attributes
    }

    pub fn into_attributes(self) -> Vec<wgpu::VertexAttribute> {
        self.attributes
    }

    pub fn size(&self) -> wgpu::BufferAddress {
        self.size
    }

    pub fn alignment(&self) -> wgpu::BufferAddress {
        self.align
    }

    pub fn next_offset(&self) -> wgpu::BufferAddress {
        self.next_offset
    }

    pub fn next_location(&self) -> u32 {
        self.location.peek()
    }

    // Procedural builder
    pub fn set_location(&mut self, location: u32) {
        self.location.jump_to(location);
    }

    pub fn set_offset(&mut self, offset: wgpu::BufferAddress) {
        self.next_offset = offset;
        self.size = self.size.max(self.next_offset);
    }

    pub fn push_alignment(&mut self, align: wgpu::BufferAddress) {
        assert!(align.is_power_of_two());

        // Update structure alignment
        self.align = self.align.max(align);

        // Update offset to be properly aligned
        self.set_offset(round_up_u64(self.next_offset(), align));
    }

    pub fn push_attribute(&mut self, attr: impl AsVertexAttribute) {
        let (format, size, align) = attr.format_size_align();

        // Align offset
        self.push_alignment(align);

        // Push attribute
        self.attributes.push(wgpu::VertexAttribute {
            format,
            offset: self.next_offset,
            shader_location: self.location.next(),
        });

        // Set offset to end of item
        self.set_offset(
            self.next_offset
                .checked_add(size)
                .expect(Self::OVERFLOW_ERR),
        );
    }

    pub fn push_sub_layout(&mut self, layout: &RawVertexBufferLayout) {
        self.push_alignment(layout.align);

        for attrib in &layout.attributes {
            self.attributes.push(wgpu::VertexAttribute {
                format: attrib.format,
                offset: self
                    .next_offset
                    .checked_add(attrib.offset)
                    .expect(Self::OVERFLOW_ERR),
                shader_location: attrib.shader_location,
            });
        }

        self.set_offset(
            self.next_offset
                .checked_add(layout.stride)
                .expect(Self::OVERFLOW_ERR),
        );
    }

    pub fn pad_to_size(&mut self, size: wgpu::BufferAddress) {
        self.size = self.size.max(size);
    }

    // Builder methods
    pub fn with_location(mut self, location: u32) -> Self {
        self.set_location(location);
        self
    }

    pub fn with_offset(mut self, offset: wgpu::BufferAddress) -> Self {
        self.set_offset(offset);
        self
    }

    pub fn with_alignment(mut self, align: wgpu::BufferAddress) -> Self {
        self.push_alignment(align);
        self
    }

    pub fn with_attribute(mut self, attr: impl AsVertexAttribute) -> Self {
        self.push_attribute(attr);
        self
    }

    pub fn with_sub_layout(mut self, layout: &RawVertexBufferLayout) -> Self {
        self.push_sub_layout(layout);
        self
    }

    pub fn with_padding_to_size(mut self, size: u64) -> Self {
        self.pad_to_size(size);
        self
    }

    pub fn finish<T>(mut self, step_mode: wgpu::VertexStepMode) -> VertexBufferLayout<T> {
        self.push_alignment(self.align);

        VertexBufferLayout::wrap(RawVertexBufferLayout {
            stride: self.size,
            align: self.align,
            step_mode,
            attributes: self.attributes,
        })
    }
}

pub trait AsVertexAttribute {
    fn format_size_align(self) -> (wgpu::VertexFormat, wgpu::BufferAddress, wgpu::BufferAddress);
}

impl AsVertexAttribute for wgpu::VertexFormat {
    fn format_size_align(self) -> (wgpu::VertexFormat, wgpu::BufferAddress, wgpu::BufferAddress) {
        (self, self.size(), 1)
    }
}

enum_index! {
    pub enum Std430VertexFormat {
        Float32,
        Float32x2,
        Float32x3,
        Float32x4,

        Float64,
        Float64x2,
        Float64x3,
        Float64x4,

        Sint32,
        Sint32x2,
        Sint32x3,
        Sint32x4,

        Uint32,
        Uint32x2,
        Uint32x3,
        Uint32x4,
    }
}

impl AsVertexAttribute for Std430VertexFormat {
    fn format_size_align(self) -> (wgpu::VertexFormat, wgpu::BufferAddress, wgpu::BufferAddress) {
        let (format, align) = match self {
            Std430VertexFormat::Float32 => (wgpu::VertexFormat::Float32, 4),
            Std430VertexFormat::Float32x2 => (wgpu::VertexFormat::Float32x2, 8),
            Std430VertexFormat::Float32x3 => (wgpu::VertexFormat::Float32x3, 16),
            Std430VertexFormat::Float32x4 => (wgpu::VertexFormat::Float32x4, 16),

            Std430VertexFormat::Float64 => (wgpu::VertexFormat::Float64, 8),
            Std430VertexFormat::Float64x2 => (wgpu::VertexFormat::Float64x2, 16),
            Std430VertexFormat::Float64x3 => (wgpu::VertexFormat::Float64x3, 32),
            Std430VertexFormat::Float64x4 => (wgpu::VertexFormat::Float64x4, 32),

            Std430VertexFormat::Sint32 => (wgpu::VertexFormat::Sint32, 4),
            Std430VertexFormat::Sint32x2 => (wgpu::VertexFormat::Sint32x2, 8),
            Std430VertexFormat::Sint32x3 => (wgpu::VertexFormat::Sint32x3, 16),
            Std430VertexFormat::Sint32x4 => (wgpu::VertexFormat::Sint32x4, 16),

            Std430VertexFormat::Uint32 => (wgpu::VertexFormat::Uint32, 4),
            Std430VertexFormat::Uint32x2 => (wgpu::VertexFormat::Uint32x2, 8),
            Std430VertexFormat::Uint32x3 => (wgpu::VertexFormat::Uint32x3, 16),
            Std430VertexFormat::Uint32x4 => (wgpu::VertexFormat::Uint32x4, 16),
        };

        (format, format.size(), align)
    }
}

#[must_use]
fn round_up_u64(value: u64, align: u64) -> u64 {
    assert!(align.is_power_of_two());
    let mask = align - 1;

    (value.saturating_add(mask)) & !mask
}
