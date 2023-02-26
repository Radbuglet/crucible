use std::any::type_name;

use crate::engine::io::gfx::GfxContext;

// === BindUniform === //

pub trait BindUniform {
	type Config<'a>;

	fn layout(config: Self::Config<'_>, builder: &mut impl BindUniformLayoutBuilder<Self>);

	fn make_wgpu_layout(config: Self::Config<'_>, gfx: &GfxContext) -> wgpu::BindGroupLayout {
		let mut builder = LayoutBuilder::default();
		<LayoutBuilder as BindUniformLayoutBuilder<Self>>::with_label(
			&mut builder,
			type_name::<Self>(),
		);
		Self::layout(config, &mut builder);
		builder.finish(gfx)
	}

	fn make_wgpu_instance(
		&self,
		config: Self::Config<'_>,
		gfx: &GfxContext,
		layout: &wgpu::BindGroupLayout,
	) -> wgpu::BindGroup {
		let mut builder = InstanceBuilder::new(self);
		Self::layout(config, &mut builder);
		builder.finish(gfx, layout)
	}
}

// TODO: Arrays
pub trait BindUniformLayoutBuilder<T: ?Sized> {
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

	fn with_storage_buffer<F>(
		&mut self,
		visibility: wgpu::ShaderStages,
		read_only: bool,
		has_dynamic_offset: bool,
		getter: F,
	) -> &mut Self
	where
		F: FnOnce(&T) -> wgpu::BufferBinding;

	fn with_sampler<F>(
		&mut self,
		visibility: wgpu::ShaderStages,
		ty: wgpu::SamplerBindingType,
		getter: F,
	) -> &mut Self
	where
		F: FnOnce(&T) -> &wgpu::Sampler;

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
}

#[derive(Default)]
struct BindSlotAssigner {
	next_slot: u32,
}

impl BindSlotAssigner {
	fn jump_to(&mut self, slot: u32) {
		self.next_slot = slot;
	}

	fn next(&mut self) -> u32 {
		let binding = self.next_slot;
		self.next_slot = self
			.next_slot
			.checked_add(1)
			.expect("Cannot create a binding at slot `u32::MAX`.");

		binding
	}
}

#[derive(Default)]
struct LayoutBuilder {
	label: Option<String>,
	binding: BindSlotAssigner,
	entries: Vec<wgpu::BindGroupLayoutEntry>,
}

impl<T: ?Sized> BindUniformLayoutBuilder<T> for LayoutBuilder {
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
}

impl LayoutBuilder {
	pub fn finish(self, gfx: &GfxContext) -> wgpu::BindGroupLayout {
		gfx.device
			.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
				label: self.label.as_deref(),
				entries: &self.entries,
			})
	}
}

struct InstanceBuilder<'a, T: ?Sized> {
	me: &'a T,
	binding: BindSlotAssigner,
	entries: Vec<wgpu::BindGroupEntry<'a>>,
}

impl<'a, T: ?Sized> InstanceBuilder<'a, T> {
	pub fn new(me: &'a T) -> Self {
		Self {
			me,
			binding: BindSlotAssigner::default(),
			entries: Vec::default(),
		}
	}
}

impl<'a, T: ?Sized> BindUniformLayoutBuilder<T> for InstanceBuilder<'a, T> {
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
}

impl<'a, T: ?Sized> InstanceBuilder<'a, T> {
	pub fn finish(self, gfx: &GfxContext, layout: &wgpu::BindGroupLayout) -> wgpu::BindGroup {
		gfx.device.create_bind_group(&wgpu::BindGroupDescriptor {
			label: None,
			layout,
			entries: &self.entries,
		})
	}
}
