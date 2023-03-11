use std::borrow::Cow;

use crucible_util::{impl_tuples, lang::marker::PhantomProlong, transparent};
use derive_where::derive_where;

use crate::{
	buffer::BufferSlice,
	pipeline::{PipelineSet, UntypedPipelineSet},
	util::SlotAssigner,
};

// === VertexBufferSet generators === //

pub trait VertexBufferSetLayoutGenerator<K: PipelineSet> {
	fn layouts(&self) -> Cow<[wgpu::VertexBufferLayout<'_>]>;
}

pub trait VertexBufferSetInstanceGenerator<K: PipelineSet> {
	fn apply<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>);
}

impl VertexBufferSetLayoutGenerator<UntypedPipelineSet> for [wgpu::VertexBufferLayout<'_>] {
	fn layouts(&self) -> Cow<[wgpu::VertexBufferLayout<'_>]> {
		self.into()
	}
}

macro_rules! impl_vertex_buffer_set {
	($($para:ident:$field:tt),*) => {
		impl<'a, $($para: 'static),*> VertexBufferSetLayoutGenerator<($($para,)*)> for ($(&'a VertexBufferLayout<$para>,)*) {
			fn layouts(&self) -> Cow<[wgpu::VertexBufferLayout<'_>]> {
				vec![$(self.$field.raw.as_wgpu()),*].into()
			}
		}

		impl<'a, $($para: 'static),*> VertexBufferSetLayoutGenerator<($($para,)*)> for ($(VertexBufferLayout<$para>,)*) {
			fn layouts(&self) -> Cow<[wgpu::VertexBufferLayout<'_>]> {
				vec![$(self.$field.raw.as_wgpu()),*].into()
			}
		}

		impl<$($para: 'static),*> VertexBufferSetInstanceGenerator<($($para,)*)> for ($(BufferSlice<'_, $para>,)*) {
			#[allow(unused)]
			fn apply<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
				let mut index = 0;
				$({
					pass.set_vertex_buffer(index, self.$field.raw);
					index += 1;
				})*
			}
		}
	};
}

impl_tuples!(impl_vertex_buffer_set; no_unit);

// === VertexBufferLayout === //

transparent! {
	#[derive_where(Debug)]
	pub struct VertexBufferLayout<T>(pub RawVertexBufferLayout, PhantomProlong<T>);
}

#[derive(Debug, Clone, Default)]
pub struct RawVertexBufferLayout {
	pub stride: wgpu::BufferAddress,
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
	const OVERFLOW_ERR: &str = "attribute offset overflowed";

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

	pub fn push_attribute(&mut self, format: wgpu::VertexFormat) {
		self.attributes.push(wgpu::VertexAttribute {
			format,
			// Vertex attributes are packed in wgpu. See the source of `wgpu::vertex_attr_array` for
			// details. FIXME: This isn't true and `wgpu` is a liar!
			offset: self.next_offset,
			shader_location: self.location.next(),
		});
		self.set_offset(
			self.next_offset
				.checked_add(format.size())
				.expect(Self::OVERFLOW_ERR),
		);
	}

	pub fn push_sub_layout(&mut self, layout: &RawVertexBufferLayout) {
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

	pub fn with_attribute(mut self, format: wgpu::VertexFormat) -> Self {
		self.push_attribute(format);
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

	pub fn finish<T>(self, step_mode: wgpu::VertexStepMode) -> VertexBufferLayout<T> {
		RawVertexBufferLayout {
			stride: self.size,
			step_mode,
			attributes: self.attributes,
		}
		.into()
	}
}
