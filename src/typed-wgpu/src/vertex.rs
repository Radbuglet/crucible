use std::{fmt, hash::Hash};

use crucible_util::{lang::marker::PhantomProlong, transparent};
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
	pub struct VertexBufferSetLayout<T>(pub RawVertexBufferSetLayout, PhantomProlong<T>);
}

#[derive(Debug, Clone)]
pub struct RawVertexBufferSetLayout {
	pub buffers: Vec<(wgpu::VertexStepMode, RawVertexBufferLayout)>,
}

pub trait VertexBufferSet {
	type CtorContext: ?Sized;
	type Config: 'static + Hash + Eq + Clone;

	fn layout(
		config: &Self::Config,
		builder: &mut impl VertexBufferSetBuilder<Self, Self::CtorContext>,
	);

	fn buffer_layouts(config: &Self::Config, ctx: &Self::CtorContext) -> RawVertexBufferSetLayout {
		let mut builder = VertexBufferSetLayoutBuilder {
			ctx,
			layouts: Vec::new(),
		};
		Self::layout(config, &mut builder);

		RawVertexBufferSetLayout {
			buffers: builder.layouts,
		}
	}

	fn apply_to_pass<'r>(&'r self, config: &Self::Config, pass: &mut wgpu::RenderPass<'r>) {
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

pub trait VertexBufferSetBuilder<T: ?Sized, C: ?Sized>: fmt::Debug {
	fn with_buffer<B, F1, F2>(
		&mut self,
		step_mode: wgpu::VertexStepMode,
		layout: F1,
		data: F2,
	) -> &mut Self
	where
		F1: FnOnce(&C) -> VertexBufferLayout<B>,
		F2: FnOnce(&T) -> BufferSlice<B>;
}

#[derive_where(Debug)]
struct VertexBufferSetLayoutBuilder<'a, C: ?Sized> {
	#[derive_where(skip)]
	ctx: &'a C,
	layouts: Vec<(wgpu::VertexStepMode, RawVertexBufferLayout)>,
}

impl<T: ?Sized, C: ?Sized> VertexBufferSetBuilder<T, C> for VertexBufferSetLayoutBuilder<'_, C> {
	fn with_buffer<B, F1, F2>(
		&mut self,
		step_mode: wgpu::VertexStepMode,
		layout: F1,
		_data: F2,
	) -> &mut Self
	where
		F1: FnOnce(&C) -> VertexBufferLayout<B>,
		F2: FnOnce(&T) -> BufferSlice<B>,
	{
		self.layouts.push((step_mode, layout(self.ctx).raw));
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

impl<'a, 'me, T: ?Sized, C: ?Sized> VertexBufferSetBuilder<T, C>
	for VertexBufferSetCommitBuilder<'a, 'me, T>
{
	fn with_buffer<B, F1, F2>(
		&mut self,
		_step_mode: wgpu::VertexStepMode,
		_layout: F1,
		data: F2,
	) -> &mut Self
	where
		F1: FnOnce(&C) -> VertexBufferLayout<B>,
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
	pub struct VertexBufferLayout<T>(pub RawVertexBufferLayout, PhantomProlong<T>);
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

// N.B. we only impl this for some arbitrary type `()` so calls to `VertexBufferLayout::builder()`
// can resolve unambiguously.
impl VertexBufferLayout<()> {
	pub fn builder() -> VertexBufferLayoutBuilder {
		VertexBufferLayoutBuilder::new()
	}
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

	// Procedural builder
	pub fn set_location(&mut self, location: u32) {
		self.location.jump_to(location);
	}

	pub fn set_offset(&mut self, offset: u64) {
		self.next_offset = offset;
		self.size = self.size.max(self.next_offset);
	}

	pub fn push_attribute(&mut self, format: wgpu::VertexFormat) {
		self.attributes.push(wgpu::VertexAttribute {
			format,
			// Vertex attributes are packed in wgpu. See the source of `wgpu::vertex_attr_array` for
			// details.
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

	pub fn pad_to_size(&mut self, size: u64) {
		self.size = self.size.max(size);
	}

	// Builder methods
	pub fn with_location(mut self, location: u32) -> Self {
		self.set_location(location);
		self
	}

	pub fn with_offset(mut self, offset: u64) -> Self {
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

	pub fn finish<T>(self) -> VertexBufferLayout<T> {
		RawVertexBufferLayout {
			stride: self.size,
			attributes: self.attributes,
		}
		.into()
	}
}
