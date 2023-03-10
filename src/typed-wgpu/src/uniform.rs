use crucible_util::{impl_tuples, lang::marker::PhantomProlong, transparent};
use derive_where::derive_where;
use std::{any::type_name, borrow::Cow, fmt, hash::Hash, num::NonZeroU32};

use crate::util::SlotAssigner;

// === UniformSet === //

// Core
transparent! {
	#[derive_where(Debug)]
	pub struct UniformSetLayout<T>(pub wgpu::PipelineLayout, PhantomProlong<T>);
}

pub trait UniformSetKind: Sized + 'static {}

pub trait UniformSetLayoutGenerator {
	type Kind: UniformSetKind;

	fn create_layout(&self, device: &wgpu::Device) -> UniformSetLayout<Self::Kind>;
}

pub trait UniformSetInstanceApplicator<K: UniformSetKind> {
	fn apply<'r>(&'r self, pass: &mut wgpu::RenderPass<'r>);
}

pub trait UniformSetInstanceCreator {
	type Kind: UniformSetKind;
	type Output;

	fn create_uniform_instance_set(self, pipeline: &wgpu::RenderPipeline) -> Self::Output;
}

pub trait BindUniformInstanceCreator {
	type Output: 'static + BindUniform;

	fn create_uniform_instance(
		self,
		layout: &wgpu::BindGroupLayout,
	) -> BindUniformInstance<Self::Output>;
}

impl<F, O> BindUniformInstanceCreator for F
where
	F: FnOnce(&wgpu::BindGroupLayout) -> BindUniformInstance<O>,
	O: 'static + BindUniform,
{
	type Output = O;

	fn create_uniform_instance(
		self,
		layout: &wgpu::BindGroupLayout,
	) -> BindUniformInstance<Self::Output> {
		self(layout)
	}
}

impl UniformSetInstanceApplicator<()> for () {
	fn apply<'r>(&'r self, _pass: &mut wgpu::RenderPass<'r>) {}
}

// Untyped kind
#[non_exhaustive]
pub struct UntypedUniformSetKind;

impl UniformSetKind for UntypedUniformSetKind {}

impl UniformSetLayoutGenerator for wgpu::PipelineLayoutDescriptor<'_> {
	type Kind = UntypedUniformSetKind;

	fn create_layout(&self, device: &wgpu::Device) -> UniformSetLayout<Self::Kind> {
		device.create_pipeline_layout(self).into()
	}
}

impl UniformSetInstanceApplicator<UntypedUniformSetKind> for [&wgpu::BindGroup] {
	fn apply<'r>(&'r self, pass: &mut wgpu::RenderPass<'r>) {
		for (index, bind_group) in self.iter().enumerate() {
			pass.set_bind_group(index as u32, bind_group, &[]);
		}
	}
}

impl UniformSetInstanceApplicator<UntypedUniformSetKind> for [(&wgpu::BindGroup, &[u32])] {
	fn apply<'r>(&'r self, pass: &mut wgpu::RenderPass<'r>) {
		for (index, (bind_group, offsets)) in self.iter().enumerate() {
			pass.set_bind_group(index as u32, bind_group, offsets);
		}
	}
}

// Typed kinds
macro_rules! impl_uniform_set {
	($($para:ident:$field:tt),*) => {
		impl<$($para: 'static + BindUniform),*> UniformSetKind for ($($para,)*) {}

		impl<$($para: 'static + BindUniform),*> UniformSetLayoutGenerator for ($(&BindUniformLayout<$para>,)*) {
			type Kind = ($($para,)*);

			fn create_layout(&self, device: &wgpu::Device) -> UniformSetLayout<Self::Kind> {
				device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
					label: Some(type_name::<Self::Kind>()),
					bind_group_layouts: &[$(&self.$field.raw,)*],
					push_constant_ranges: &[],
				}).into()
			}
		}

		impl<$($para: BindUniformInstanceCreator),*> UniformSetInstanceCreator for ($($para,)*) {
			type Kind = ($($para::Output,)*);
			type Output = ($(BindUniformInstance<$para::Output>,)*);

			#[allow(unused)]
			fn create_uniform_instance_set(
				self,
				pipeline: &wgpu::RenderPipeline,
			) -> Self::Output {
				let mut index = 0;

				($(
					self.$field.create_uniform_instance(&pipeline.get_bind_group_layout({
						let old = index;
						index += 1;
						old
					})),
				)*)
			}
		}
	};
}

macro_rules! impl_uniform_set_no_unit {
	($($para:ident:$field:tt),*) => {
		impl<$($para: 'static + BindUniform<DynamicOffsets = NoDynamicOffsets>),*>
			UniformSetInstanceApplicator<($($para,)*)> for
			($(&BindUniformInstance<$para>,)*)
		{
			#[allow(unused)]
			fn apply<'r>(&'r self, pass: &mut wgpu::RenderPass<'r>) {
				let mut index = 0;
				$({
					pass.set_bind_group(index, &self.$field.raw, &[]);
					index += 1;
				})*
			}
		}

		impl<$($para: 'static + BindUniform<DynamicOffsets = NoDynamicOffsets>),*>
			UniformSetInstanceApplicator<($($para,)*)> for
			($(BindUniformInstance<$para>,)*)
		{
			#[allow(unused)]
			fn apply<'r>(&'r self, pass: &mut wgpu::RenderPass<'r>) {
				let mut index = 0;
				$({
					pass.set_bind_group(index, &self.$field.raw, &[]);
					index += 1;
				})*
			}
		}

		impl<$($para: 'static + BindUniform),*>
			UniformSetInstanceApplicator<($($para,)*)> for
			($((
				&BindUniformInstance<$para>,
				$para::DynamicOffsets,
			),)*)
		{
			#[allow(unused)]
			fn apply<'r>(&'r self, pass: &mut wgpu::RenderPass<'r>) {
				let mut index = 0;
				$({
					pass.set_bind_group(index, &self.$field.0.raw, &*self.$field.1.as_offset_set());
					index += 1;
				})*
			}
		}
	};
}

impl_tuples!(impl_uniform_set);
impl_tuples!(impl_uniform_set_no_unit; no_unit);

// === BindUniform === //

transparent! {
	#[derive_where(Debug)]
	pub struct BindUniformLayout<T>(pub wgpu::BindGroupLayout, PhantomProlong<T>);

	#[derive_where(Debug)]
	pub struct BindUniformInstance<T>(pub wgpu::BindGroup, PhantomProlong<T>);
}

pub trait BindUniform: Sized {
	type Config: 'static + Hash + Eq + Clone;
	type DynamicOffsets: DynamicOffsetSet;

	fn layout(builder: &mut impl BindUniformBuilder<Self>, config: &Self::Config);

	fn create_layout(device: &wgpu::Device, config: &Self::Config) -> BindUniformLayout<Self> {
		let mut builder = BindUniformLayoutBuilder::default();
		<BindUniformLayoutBuilder as BindUniformBuilder<Self>>::with_label(
			&mut builder,
			type_name::<Self>(),
		);
		Self::layout(&mut builder, config);

		builder.finish(device).into()
	}

	fn create_instance(
		&self,
		device: &wgpu::Device,
		layout: &wgpu::BindGroupLayout,
		config: &Self::Config,
	) -> BindUniformInstance<Self> {
		let mut builder = BindUniformInstanceBuilder::new(self);
		Self::layout(&mut builder, config);

		builder.finish(device, layout).into()
	}
}

pub trait BindUniformBuilder<T: ?Sized>: fmt::Debug {
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
struct BindUniformLayoutBuilder {
	label: Option<String>,
	binding: SlotAssigner,
	entries: Vec<wgpu::BindGroupLayoutEntry>,
}

impl<T: ?Sized> BindUniformBuilder<T> for BindUniformLayoutBuilder {
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

impl BindUniformLayoutBuilder {
	pub fn finish(self, device: &wgpu::Device) -> wgpu::BindGroupLayout {
		device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			label: self.label.as_deref(),
			entries: &self.entries,
		})
	}
}

#[derive_where(Debug)]
struct BindUniformInstanceBuilder<'a, T: ?Sized> {
	#[derive_where(skip)]
	me: &'a T,
	binding: SlotAssigner,
	entries: Vec<wgpu::BindGroupEntry<'a>>,
}

impl<'a, T: ?Sized> BindUniformInstanceBuilder<'a, T> {
	pub fn new(me: &'a T) -> Self {
		Self {
			me,
			binding: SlotAssigner::default(),
			entries: Vec::default(),
		}
	}
}

impl<'a, T: ?Sized> BindUniformBuilder<T> for BindUniformInstanceBuilder<'a, T> {
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

impl<'a, T: ?Sized> BindUniformInstanceBuilder<'a, T> {
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
	fn as_offset_set(&self) -> Cow<[wgpu::DynamicOffset]>;
}

impl DynamicOffsetSet for [wgpu::DynamicOffset] {
	fn as_offset_set(&self) -> Cow<[wgpu::DynamicOffset]> {
		self.into()
	}
}

impl<const N: usize> DynamicOffsetSet for [wgpu::DynamicOffset; N] {
	fn as_offset_set(&self) -> Cow<[wgpu::DynamicOffset]> {
		self.as_slice().into()
	}
}

pub type NoDynamicOffsets = [wgpu::DynamicOffset; 0];

// === PushUniform === //

// TODO
