use crucible_util::lang::iter::VolumetricIter;
use hashbrown::HashSet;
use image::{imageops, Rgba32FImage};
use typed_glam::glam::{UVec2, Vec2};

use crate::engine::{gfx::texture::write_texture_data_raw, io::gfx::GfxContext};

#[derive(Debug)]
pub struct AtlasTexture {
	tile_size: UVec2,
	tile_counts: UVec2,
	free_tiles: HashSet<UVec2>,
	atlas: Vec<Rgba32FImage>,
}

impl AtlasTexture {
	pub fn new(tile_size: UVec2, tile_counts: UVec2, mips: u32) -> Self {
		let image_size = tile_size * tile_counts;

		Self {
			tile_size,
			tile_counts,
			free_tiles: VolumetricIter::new_exclusive_iter(tile_counts.to_array())
				.map(UVec2::from_array)
				.collect::<HashSet<_>>(),
			atlas: (0..mips)
				.map(|level| {
					let size = wgpu::Extent3d {
						width: image_size.x,
						height: image_size.y,
						depth_or_array_layers: 1,
					}
					.mip_level_size(level, wgpu::TextureDimension::D2);

					Rgba32FImage::new(size.width, size.height)
				})
				.collect(),
		}
	}

	pub fn textures(&self) -> &[Rgba32FImage] {
		&self.atlas
	}

	pub fn mips_layers(&self) -> u32 {
		self.textures().len() as u32
	}

	pub fn tile_size(&self) -> UVec2 {
		self.tile_size
	}

	pub fn tile_counts(&self) -> UVec2 {
		self.tile_counts
	}

	pub fn atlas_size(&self) -> UVec2 {
		UVec2::new(self.atlas[0].width(), self.atlas[0].height())
	}

	pub fn free_tile_count(&self) -> usize {
		self.free_tiles.len()
	}

	pub fn max_tile_count(&self) -> usize {
		(self.tile_counts.x * self.tile_counts.y) as usize
	}

	pub fn is_empty(&self) -> bool {
		self.free_tile_count() == self.max_tile_count()
	}

	pub fn is_full(&self) -> bool {
		self.free_tile_count() == 0
	}

	pub fn add(&mut self, sub: &Rgba32FImage) -> UVec2 {
		debug_assert_eq!(sub.width(), self.tile_size.x);
		debug_assert_eq!(sub.height(), self.tile_size.y);

		// Allocate a free tile
		let free_tile = *self
			.free_tiles
			.iter()
			.next()
			.expect("no free tiles in atlas");

		self.free_tiles.remove(&free_tile);

		// Write to the free tile
		let atlas_size = self.atlas_size().as_dvec2();
		let offset = free_tile * self.tile_size;

		for layer in &mut self.atlas {
			let factor_x = layer.width() as f64 / atlas_size.x;
			let factor_y = layer.height() as f64 / atlas_size.y;

			imageops::replace(
				layer,
				&imageops::resize(
					sub,
					(sub.width() as f64 * factor_x) as u32,
					(sub.height() as f64 * factor_y) as u32,
					imageops::FilterType::Gaussian,
				),
				(offset.x as f64 * factor_x) as i64,
				(offset.y as f64 * factor_y) as i64,
			);
		}

		free_tile
	}

	pub fn remove(&mut self, sub: UVec2) {
		debug_assert!(sub.x < self.tile_counts.x);
		debug_assert!(sub.y < self.tile_counts.y);

		self.free_tiles.insert(sub);
	}

	pub fn decode_uv_percent_bounds(&self, tile: UVec2) -> (Vec2, Vec2) {
		let (origin, size) = self.decode_uv_pixel_bounds(tile);
		let tex_size = self.atlas_size().as_vec2();
		(origin / tex_size, size / tex_size)
	}

	pub fn decode_uv_pixel_bounds(&self, tile: UVec2) -> (Vec2, Vec2) {
		let origin = tile.as_vec2() * self.tile_size.as_vec2();
		let size = self.tile_size.as_vec2();
		(origin, size)
	}
}

#[derive(Debug)]
pub struct AtlasTextureGfx {
	pub texture: wgpu::Texture,
	pub view: wgpu::TextureView,
	pub tex_dim: UVec2,
}

impl AtlasTextureGfx {
	pub fn new(gfx: &GfxContext, atlas: &AtlasTexture, label: Option<&str>) -> Self {
		let tex_dim = atlas.atlas_size();
		let texture = gfx.device.create_texture(&wgpu::TextureDescriptor {
			label,
			size: wgpu::Extent3d {
				width: tex_dim.x,
				height: tex_dim.y,
				depth_or_array_layers: 1,
			},
			mip_level_count: atlas.mips_layers(),
			sample_count: 1,
			dimension: wgpu::TextureDimension::D2,
			format: wgpu::TextureFormat::Rgba32Float,
			usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
			view_formats: &[],
		});
		let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

		Self {
			texture,
			view,
			tex_dim,
		}
	}

	pub fn update(&mut self, gfx: &GfxContext, atlas: &AtlasTexture) {
		let dim = atlas.atlas_size();
		debug_assert_eq!(dim, self.tex_dim);

		let mut mips_data = Vec::new();
		for layer in atlas.textures() {
			mips_data.extend(bytemuck::cast_slice(layer));
		}

		write_texture_data_raw(gfx, &self.texture, &mips_data);
	}
}
