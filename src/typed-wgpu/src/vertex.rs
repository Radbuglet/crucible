use std::{fmt, hash::Hash};

use crucible_util::transparent;
use derive_where::derive_where;

use crate::{buffer::BufferSlice, util::SlotAssigner};

// === VertexShader === //

#[derive_where(Debug)]
pub struct VertexShader<V> {
	pub module: wgpu::ShaderModule,
	pub entry_point: String,
	pub vertex_layout: VertexBufferSetLayout<V>,
}

// === VertexBufferSet === //

transparent! {
	#[derive_where(Debug, Clone)]
	pub struct VertexBufferSetLayout<T>(pub RawVertexBufferSetLayout, T);
}

#[derive(Debug, Clone)]
pub struct RawVertexBufferSetLayout {
	pub buffers: Vec<(wgpu::VertexStepMode, RawVertexBufferLayout)>,
}

pub trait VertexBufferSet {
	type Config: 'static + Hash + Eq + Clone;

	fn layout(config: &Self::Config, builder: &mut impl VertexBufferSetBuilder<Self>);

	fn buffer_layouts(config: &Self::Config) -> RawVertexBufferSetLayout {
		let mut builder = VertexBufferSetLayoutBuilder::default();
		Self::layout(config, &mut builder);

		RawVertexBufferSetLayout {
			buffers: builder.layouts,
		}
	}

	fn apply<'r>(&'r self, config: &Self::Config, pass: &mut wgpu::RenderPass<'r>) {
		Self::layout(
			config,
			&mut VertexBufferSetCommitBuilder {
				me: self,
				pass,
				slot: 0,
			},
		);
	}
}

pub trait VertexBufferSetBuilder<T: ?Sized>: fmt::Debug {
	fn with_buffer<B, F1, F2>(
		&mut self,
		step_mode: wgpu::VertexStepMode,
		layout: F1,
		data: F2,
	) -> &mut Self
	where
		F1: FnOnce() -> VertexBufferLayout<B>,
		F2: FnOnce(&T) -> BufferSlice<B>;
}

#[derive(Debug, Default)]
struct VertexBufferSetLayoutBuilder {
	layouts: Vec<(wgpu::VertexStepMode, RawVertexBufferLayout)>,
}

impl<T: ?Sized> VertexBufferSetBuilder<T> for VertexBufferSetLayoutBuilder {
	fn with_buffer<B, F1, F2>(
		&mut self,
		step_mode: wgpu::VertexStepMode,
		layout: F1,
		_data: F2,
	) -> &mut Self
	where
		F1: FnOnce() -> VertexBufferLayout<B>,
		F2: FnOnce(&T) -> BufferSlice<B>,
	{
		self.layouts.push((step_mode, layout().raw));
		self
	}
}

#[derive_where(Debug)]
struct VertexBufferSetCommitBuilder<'a, 'me, T: ?Sized> {
	#[derive_where(skip)]
	me: &'me T,
	pass: &'a mut wgpu::RenderPass<'me>,
	slot: u32,
}

impl<'a, 'me, T: ?Sized> VertexBufferSetBuilder<T> for VertexBufferSetCommitBuilder<'a, 'me, T> {
	fn with_buffer<B, F1, F2>(
		&mut self,
		_step_mode: wgpu::VertexStepMode,
		_layout: F1,
		data: F2,
	) -> &mut Self
	where
		F1: FnOnce() -> VertexBufferLayout<B>,
		F2: FnOnce(&T) -> BufferSlice<B>,
	{
		self.pass.set_vertex_buffer(self.slot, data(self.me).raw);
		self.slot = self
			.slot
			.checked_add(1)
			.expect("VertexBufferSetBuilder slot counter overflowed.");
		self
	}
}

// === VertexBufferLayout === //

transparent! {
	#[derive_where(Debug)]
	pub struct VertexBufferLayout<T>(pub RawVertexBufferLayout, T);
}

#[derive(Debug, Clone, Default)]
pub struct RawVertexBufferLayout {
	pub stride: u64,
	pub attributes: Vec<wgpu::VertexAttribute>,
}

#[derive(Debug, Clone, Default)]
pub struct VertexBufferLayoutBuilder {
	location: SlotAssigner,
	size: u64,
	next_offset: u64,
	attributes: Vec<wgpu::VertexAttribute>,
}

// TODO: Check for duplicates and overlap?
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

	pub fn size(&self) -> u64 {
		self.size
	}

	pub fn next_offset(&self) -> u64 {
		self.next_offset
	}

	pub fn next_location(&self) -> u32 {
		self.location.peek()
	}

	// Builders
	pub fn with_location(&mut self, location: u32) -> &mut Self {
		self.location.jump_to(location);
		self
	}

	pub fn with_offset(&mut self, offset: u64) -> &mut Self {
		self.next_offset = offset;
		self.size = self.size.max(self.next_offset);
		self
	}

	pub fn with_attribute(&mut self, format: wgpu::VertexFormat) -> &mut Self {
		self.attributes.push(wgpu::VertexAttribute {
			format,
			// Vertex attributes are packed in wgpu. See the source of `wgpu::vertex_attr_array` for
			// details.
			offset: self.next_offset,
			shader_location: self.location.next(),
		});
		self.with_offset(
			self.next_offset
				.checked_add(format.size())
				.expect(Self::OVERFLOW_ERR),
		);
		self
	}

	pub fn with_sub_layout(&mut self, layout: &RawVertexBufferLayout) -> &mut Self {
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

		self.with_offset(
			self.next_offset
				.checked_add(layout.stride)
				.expect(Self::OVERFLOW_ERR),
		);
		self
	}

	pub fn with_pad_to_size(&mut self, size: u64) -> &mut Self {
		self.size = self.size.max(size);
		self
	}

	pub fn finish<T>(self) -> VertexBufferLayout<T> {
		RawVertexBufferLayout {
			stride: self.size,
			attributes: self.attributes,
		}
		.into()
	}
}
