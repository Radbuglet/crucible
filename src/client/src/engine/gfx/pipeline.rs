use std::{any::type_name, num::NonZeroU64};

use crucible_util::delegate;
use derive_where::derive_where;

use crate::engine::io::gfx::GfxContext;

// === BindUniform === //

#[derive_where(Debug, Default)]
pub struct BindUniformLayoutBuilder<'a, T> {
	label: Option<&'a str>,
	entries: Vec<BindUniformElement<T>>,
}

impl<'a, T> BindUniformLayoutBuilder<'a, T> {
	// TODO: Allow users to configure binding indices

	pub fn entries(&self) -> &Vec<BindUniformElement<T>> {
		&self.entries
	}

	pub fn entries_mut(&mut self) -> &mut Vec<BindUniformElement<T>> {
		&mut self.entries
	}

	pub fn label(&self) -> Option<&'a str> {
		self.label
	}

	pub fn with_label(mut self, label: &'a str) -> Self {
		self.label = Some(label);
		self
	}

	pub fn with_uniform_buffer<F>(
		mut self,
		stages: wgpu::ShaderStages,
		has_dynamic_offset: bool,
		getter: F,
	) -> Self
	where
		F: 'static + Send + Sync + Fn(&T) -> wgpu::BufferBinding,
	{
		self.entries.push(BindUniformElement::Buffer {
			stages,
			ty: wgpu::BufferBindingType::Uniform,
			has_dynamic_offset,
			min_binding_size: None,
			getter: BindUniformBufferGetter::new(getter),
		});
		self
	}

	pub fn with_storage_buffer<F>(
		mut self,
		stages: wgpu::ShaderStages,
		read_only: bool,
		has_dynamic_offset: bool,
		getter: F,
	) -> Self
	where
		F: 'static + Send + Sync + Fn(&T) -> wgpu::BufferBinding,
	{
		self.entries.push(BindUniformElement::Buffer {
			stages,
			ty: wgpu::BufferBindingType::Storage { read_only },
			has_dynamic_offset,
			min_binding_size: None,
			getter: BindUniformBufferGetter::new(getter),
		});
		self
	}

	pub fn with_sampler<F>(
		mut self,
		stages: wgpu::ShaderStages,
		ty: wgpu::SamplerBindingType,
		getter: F,
	) -> Self
	where
		F: 'static + Send + Sync + Fn(&T) -> &wgpu::Sampler,
	{
		self.entries.push(BindUniformElement::Sampler {
			stages,
			ty,
			getter: BindUniformSamplerGetter::new(getter),
		});
		self
	}

	pub fn with_texture<F>(
		mut self,
		stages: wgpu::ShaderStages,
		sample_type: wgpu::TextureSampleType,
		view_dimension: wgpu::TextureViewDimension,
		multisampled: bool,
		getter: F,
	) -> Self
	where
		F: 'static + Send + Sync + Fn(&T) -> &wgpu::TextureView,
	{
		self.entries.push(BindUniformElement::Texture {
			stages,
			sample_type,
			view_dimension,
			multisampled,
			getter: BindUniformTextureGetter::new(getter),
		});
		self
	}

	pub fn with_storage_texture<F>(
		mut self,
		stages: wgpu::ShaderStages,
		access: wgpu::StorageTextureAccess,
		format: wgpu::TextureFormat,
		view_dimension: wgpu::TextureViewDimension,
		getter: F,
	) -> Self
	where
		F: 'static + Send + Sync + Fn(&T) -> &wgpu::TextureView,
	{
		self.entries.push(BindUniformElement::StorageTexture {
			stages,
			access,
			format,
			view_dimension,
			getter: BindUniformTextureGetter::new(getter),
		});
		self
	}

	pub fn build(self, gfx: &GfxContext) -> BindUniformLayout<T> {
		let entries = self
			.entries
			.iter()
			.enumerate()
			.map(|(binding, entry)| entry.as_layout_entry(binding as u32))
			.collect::<Vec<_>>();

		let layout = gfx
			.device
			.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
				label: Some(self.label.unwrap_or(type_name::<T>())),
				entries: entries.as_slice(),
			});

		BindUniformLayout {
			layout,
			entries: self.entries.into_boxed_slice(),
		}
	}
}

#[derive_where(Debug)]
pub enum BindUniformElement<T> {
	Buffer {
		stages: wgpu::ShaderStages,
		ty: wgpu::BufferBindingType,
		has_dynamic_offset: bool,
		min_binding_size: Option<NonZeroU64>,
		getter: BindUniformBufferGetter<T>,
	},
	Sampler {
		stages: wgpu::ShaderStages,
		ty: wgpu::SamplerBindingType,
		getter: BindUniformSamplerGetter<T>,
	},
	Texture {
		stages: wgpu::ShaderStages,
		sample_type: wgpu::TextureSampleType,
		view_dimension: wgpu::TextureViewDimension,
		multisampled: bool,
		getter: BindUniformTextureGetter<T>,
	},
	StorageTexture {
		stages: wgpu::ShaderStages,
		access: wgpu::StorageTextureAccess,
		format: wgpu::TextureFormat,
		view_dimension: wgpu::TextureViewDimension,
		getter: BindUniformTextureGetter<T>,
	},
}

impl<T> BindUniformElement<T> {
	pub fn as_layout_entry(&self, binding: u32) -> wgpu::BindGroupLayoutEntry {
		match self {
			BindUniformElement::Buffer {
				stages,
				ty,
				has_dynamic_offset,
				min_binding_size,
				..
			} => wgpu::BindGroupLayoutEntry {
				binding,
				visibility: *stages,
				ty: wgpu::BindingType::Buffer {
					ty: *ty,
					has_dynamic_offset: *has_dynamic_offset,
					min_binding_size: *min_binding_size,
				},
				count: None,
			},
			BindUniformElement::Sampler { stages, ty, .. } => wgpu::BindGroupLayoutEntry {
				binding,
				visibility: *stages,
				ty: wgpu::BindingType::Sampler(*ty),
				count: None,
			},
			BindUniformElement::Texture {
				stages,
				sample_type,
				view_dimension,
				multisampled,
				..
			} => wgpu::BindGroupLayoutEntry {
				binding,
				visibility: *stages,
				ty: wgpu::BindingType::Texture {
					sample_type: *sample_type,
					view_dimension: *view_dimension,
					multisampled: *multisampled,
				},
				count: None,
			},
			BindUniformElement::StorageTexture {
				stages,
				access,
				format,
				view_dimension,
				..
			} => wgpu::BindGroupLayoutEntry {
				binding,
				visibility: *stages,
				ty: wgpu::BindingType::StorageTexture {
					access: *access,
					format: *format,
					view_dimension: *view_dimension,
				},
				count: None,
			},
		}
	}

	pub fn as_binding_resource<'a>(&'a self, state: &'a T) -> wgpu::BindingResource<'a> {
		// TODO: How do we handle arrays?
		match self {
			BindUniformElement::Buffer { getter, .. } => {
				wgpu::BindingResource::Buffer(getter(state))
			}
			BindUniformElement::Sampler { getter, .. } => {
				wgpu::BindingResource::Sampler(getter(state))
			}
			BindUniformElement::Texture { getter, .. } => {
				wgpu::BindingResource::TextureView(getter(state))
			}
			BindUniformElement::StorageTexture { getter, .. } => {
				wgpu::BindingResource::TextureView(getter(state))
			}
		}
	}
}

delegate! {
	pub fn BindUniformBufferGetter<T>(src: &T) -> wgpu::BufferBinding
}

delegate! {
	pub fn BindUniformSamplerGetter<T>(src: &T) -> &wgpu::Sampler
}

delegate! {
	pub fn BindUniformTextureGetter<T>(src: &T) -> &wgpu::TextureView
}

#[derive_where(Debug)]
pub struct BindUniformLayout<T> {
	layout: wgpu::BindGroupLayout,
	entries: Box<[BindUniformElement<T>]>,
}

impl<T> BindUniformLayout<T> {
	pub fn wgpu(&self) -> &wgpu::BindGroupLayout {
		&self.layout
	}

	pub fn entries(&self) -> &[BindUniformElement<T>] {
		&self.entries
	}

	pub fn make_group(&self, gfx: &GfxContext, label: Option<&str>, state: &T) -> wgpu::BindGroup {
		let entries = self
			.entries
			.iter()
			.enumerate()
			.map(|(binding, entry)| wgpu::BindGroupEntry {
				binding: binding as u32,
				resource: entry.as_binding_resource(state),
			})
			.collect::<Vec<_>>();

		gfx.device.create_bind_group(&wgpu::BindGroupDescriptor {
			label,
			layout: &self.layout,
			entries: entries.as_slice(),
		})
	}
}
