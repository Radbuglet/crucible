use std::borrow::{Borrow, Cow};

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

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct SamplerAssetDescriptor {
	pub label: ReifiedDebugLabel,
	pub address_mode_u: wgpu::AddressMode,
	pub address_mode_v: wgpu::AddressMode,
	pub address_mode_w: wgpu::AddressMode,
	pub mag_filter: wgpu::FilterMode,
	pub min_filter: wgpu::FilterMode,
	pub mipmap_filter: wgpu::FilterMode,
	pub compare: Option<wgpu::CompareFunction>,
	pub anisotropy_clamp: u16,
	pub border_color: Option<wgpu::SamplerBorderColor>,
}

impl SamplerAssetDescriptor {
	pub const NEAREST_CLAMP_EDGES: Self = Self {
		label: Some(Cow::Borrowed("nearest clamp edges")),
		address_mode_u: wgpu::AddressMode::ClampToEdge,
		address_mode_v: wgpu::AddressMode::ClampToEdge,
		address_mode_w: wgpu::AddressMode::ClampToEdge,
		mag_filter: wgpu::FilterMode::Nearest,
		min_filter: wgpu::FilterMode::Nearest,
		mipmap_filter: wgpu::FilterMode::Nearest,
		compare: None,
		anisotropy_clamp: 0,
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
				lod_min_clamp: 0.0,
				lod_max_clamp: f32::MAX,
				compare: self.compare,
				anisotropy_clamp: self.anisotropy_clamp,
				border_color: self.border_color,
			})
		})
	}
}
