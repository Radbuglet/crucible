use std::{
	borrow::{Borrow, Cow},
	hash,
};

use bort::CompRef;
use crucible_util::debug::label::{DebugLabel, ReifiedDebugLabel};
use typed_glam::glam::UVec2;

use crate::engine::{
	assets::AssetManager,
	io::{gfx::GfxContext, viewport::Viewport},
};

// === FullScreenTexture === //

#[derive(Debug)]
pub struct FullScreenTexture {
	texture: Option<(wgpu::Texture, wgpu::TextureView)>,
	conf_label: ReifiedDebugLabel,
	conf_size: UVec2,
	conf_format: wgpu::TextureFormat,
	conf_usages: wgpu::TextureUsages,
	conf_dirty: bool,
}

impl FullScreenTexture {
	pub fn new(
		label: impl DebugLabel,
		format: wgpu::TextureFormat,
		usages: wgpu::TextureUsages,
	) -> Self {
		Self {
			texture: None,
			conf_label: label.reify(),
			conf_size: UVec2::ZERO,
			conf_format: format,
			conf_usages: usages,
			conf_dirty: false,
		}
	}

	pub fn label(&self) -> &ReifiedDebugLabel {
		&self.conf_label
	}

	pub fn set_label(&mut self, label: impl DebugLabel) {
		self.conf_label = label.reify();
		self.conf_dirty = true;
	}

	pub fn format(&self) -> wgpu::TextureFormat {
		self.conf_format
	}

	pub fn set_format(&mut self, format: wgpu::TextureFormat) {
		if self.conf_format != format {
			self.conf_format = format;
			self.conf_dirty = true;
		}
	}

	pub fn usages(&self) -> wgpu::TextureUsages {
		self.conf_usages
	}

	pub fn set_usages(&mut self, usages: wgpu::TextureUsages) {
		if self.conf_usages != usages {
			self.conf_usages = usages;
			self.conf_dirty = true;
		}
	}

	pub fn wgpu_descriptor(&self) -> wgpu::TextureDescriptor {
		wgpu::TextureDescriptor {
			label: self.conf_label.as_ref().map(Borrow::borrow),
			size: wgpu::Extent3d {
				width: self.conf_size.x,
				height: self.conf_size.y,
				depth_or_array_layers: 1,
			},
			mip_level_count: 1,
			sample_count: 1,
			dimension: wgpu::TextureDimension::D2,
			format: self.conf_format,
			usage: self.conf_usages,
			view_formats: &[],
		}
	}

	pub fn acquire(
		&mut self,
		gfx: &GfxContext,
		viewport: &Viewport,
	) -> Option<(&mut wgpu::Texture, &mut wgpu::TextureView)> {
		if let Some(curr_size) = viewport.curr_surface_size() {
			// Look for a size mismatch
			if curr_size != self.conf_size {
				self.conf_size = curr_size;
				self.conf_dirty = true;
			}

			// Rebuild the texture if the config is dirty or if the texture is not yet bound.
			if self.conf_dirty || self.texture.is_none() {
				let texture = gfx.device.create_texture(&self.wgpu_descriptor());
				let view = texture.create_view(&wgpu::TextureViewDescriptor {
					label: self.conf_label.as_ref().map(Borrow::borrow),
					..Default::default()
				});
				self.texture = Some((texture, view));
				self.conf_dirty = false;
			}
		} else {
			self.texture = None;
		}

		self.texture.as_mut().map(|(tex, view)| (tex, view))
	}

	pub fn acquire_view(
		&mut self,
		gfx: &GfxContext,
		viewport: &Viewport,
	) -> &mut wgpu::TextureView {
		self.acquire(gfx, viewport).unwrap().1
	}
}

// === SamplerDesc === //

#[derive(Debug, Clone, PartialEq)]
pub struct SamplerAssetDescriptor {
	pub label: ReifiedDebugLabel,
	pub address_mode_u: wgpu::AddressMode,
	pub address_mode_v: wgpu::AddressMode,
	pub address_mode_w: wgpu::AddressMode,
	pub mag_filter: wgpu::FilterMode,
	pub min_filter: wgpu::FilterMode,
	pub mipmap_filter: wgpu::FilterMode,
	pub lod_min_clamp: f32,
	pub lod_max_clamp: f32,
	pub compare: Option<wgpu::CompareFunction>,
	pub anisotropy_clamp: u16,
	pub border_color: Option<wgpu::SamplerBorderColor>,
}

impl hash::Hash for SamplerAssetDescriptor {
	fn hash<H: hash::Hasher>(&self, state: &mut H) {
		self.label.hash(state);
		self.address_mode_u.hash(state);
		self.address_mode_v.hash(state);
		self.address_mode_w.hash(state);
		self.mag_filter.hash(state);
		self.min_filter.hash(state);
		self.mipmap_filter.hash(state);
		self.lod_min_clamp.to_bits().hash(state);
		self.lod_max_clamp.to_bits().hash(state);
		self.compare.hash(state);
		self.anisotropy_clamp.hash(state);
		self.border_color.hash(state);
	}
}

impl Eq for SamplerAssetDescriptor {}

impl SamplerAssetDescriptor {
	pub const NEAREST_CLAMP_EDGES: Self = Self {
		label: Some(Cow::Borrowed("nearest clamp edges")),
		address_mode_u: wgpu::AddressMode::ClampToEdge,
		address_mode_v: wgpu::AddressMode::ClampToEdge,
		address_mode_w: wgpu::AddressMode::ClampToEdge,
		mag_filter: wgpu::FilterMode::Nearest,
		min_filter: wgpu::FilterMode::Nearest,
		mipmap_filter: wgpu::FilterMode::Linear,
		lod_min_clamp: 0.0,
		lod_max_clamp: 0.0,
		compare: None,
		anisotropy_clamp: 1,
		border_color: None,
	};

	pub const FILTER_CLAMP_EDGES: Self = Self {
		label: Some(Cow::Borrowed("filter clamp edges")),
		address_mode_u: wgpu::AddressMode::ClampToEdge,
		address_mode_v: wgpu::AddressMode::ClampToEdge,
		address_mode_w: wgpu::AddressMode::ClampToEdge,
		mag_filter: wgpu::FilterMode::Linear,
		min_filter: wgpu::FilterMode::Linear,
		mipmap_filter: wgpu::FilterMode::Linear,
		lod_min_clamp: 0.0,
		lod_max_clamp: 0.0,
		compare: None,
		anisotropy_clamp: 1,
		border_color: None,
	};
}

impl Default for SamplerAssetDescriptor {
	fn default() -> Self {
		Self::NEAREST_CLAMP_EDGES
	}
}

impl SamplerAssetDescriptor {
	pub fn load(
		&self,
		assets: &mut AssetManager,
		gfx: &GfxContext,
	) -> CompRef<'static, wgpu::Sampler> {
		assets.cache(self, |_| {
			gfx.device.create_sampler(&wgpu::SamplerDescriptor {
				label: self.label.as_ref().map(Borrow::borrow),
				address_mode_u: self.address_mode_u,
				address_mode_v: self.address_mode_v,
				address_mode_w: self.address_mode_w,
				mag_filter: self.mag_filter,
				min_filter: self.min_filter,
				mipmap_filter: self.mipmap_filter,
				lod_min_clamp: self.lod_min_clamp,
				lod_max_clamp: self.lod_max_clamp,
				compare: self.compare,
				anisotropy_clamp: self.anisotropy_clamp,
				border_color: self.border_color,
			})
		})
	}
}

// === Texture Uploads === //

// This is largely stolen from wgpu's own `create_texture_with_data` method.
// Source: https://github.com/gfx-rs/wgpu/blob/e47dc2adadbf040c8cdb0ee21e602d8d772f8515/wgpu/src/util/device.rs#L77-L144
pub fn write_texture_data_raw(gfx: &GfxContext, texture: &wgpu::Texture, data: &[u8]) {
	// Will return None only if it's a combined depth-stencil format
	// If so, default to 4, validation will fail later anyway since the depth or stencil
	// aspect needs to be written to individually
	let block_size = texture.format().block_size(None).unwrap_or(4);
	let (block_width, block_height) = texture.format().block_dimensions();
	let layer_iterations = texture.depth_or_array_layers();

	let mut binary_offset = 0;
	for layer in 0..layer_iterations {
		for mip in 0..texture.mip_level_count() {
			let mut mip_size = texture.size().mip_level_size(mip, texture.dimension());
			// copying layers separately
			if texture.dimension() != wgpu::TextureDimension::D3 {
				mip_size.depth_or_array_layers = 1;
			}

			// When uploading mips of compressed textures and the mip is supposed to be
			// a size that isn't a multiple of the block size, the mip needs to be uploaded
			// as its "physical size" which is the size rounded up to the nearest block size.
			let mip_physical = mip_size.physical_size(texture.format());

			// All these calculations are performed on the physical size as that's the
			// data that exists in the buffer.
			let width_blocks = mip_physical.width / block_width;
			let height_blocks = mip_physical.height / block_height;

			let bytes_per_row = width_blocks * block_size;
			let data_size = bytes_per_row * height_blocks * mip_size.depth_or_array_layers;

			let end_offset = binary_offset + data_size as usize;

			gfx.queue.write_texture(
				wgpu::ImageCopyTexture {
					texture: &texture,
					mip_level: mip,
					origin: wgpu::Origin3d {
						x: 0,
						y: 0,
						z: layer,
					},
					aspect: wgpu::TextureAspect::All,
				},
				&data[binary_offset..end_offset],
				wgpu::ImageDataLayout {
					offset: 0,
					bytes_per_row: Some(bytes_per_row),
					rows_per_image: Some(height_blocks),
				},
				mip_physical,
			);

			binary_offset = end_offset;
		}
	}
}
