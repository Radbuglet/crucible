use crucible_util::lang::iter::VolumetricIter;
use hashbrown::HashSet;
use image::{imageops, Rgba32FImage};
use typed_glam::glam::{UVec2, Vec2};

use crate::engine::io::gfx::GfxContext;

#[derive(Debug)]
pub struct AtlasTexture {
	tile_size: UVec2,
	tile_counts: UVec2,
	free_tiles: HashSet<UVec2>,
	atlas: Rgba32FImage,
}

impl AtlasTexture {
	pub fn new(tile_size: UVec2, tile_counts: UVec2) -> Self {
		let image_size = tile_size * tile_counts;

		Self {
			tile_size,
			tile_counts,
			free_tiles: VolumetricIter::new_exclusive_iter(tile_counts.to_array())
				.map(UVec2::from_array)
				.collect::<HashSet<_>>(),
			atlas: Rgba32FImage::new(image_size.x, image_size.y),
		}
	}

	pub fn texture(&self) -> &Rgba32FImage {
		&self.atlas
	}

	pub fn tile_size(&self) -> UVec2 {
		self.tile_size
	}

	pub fn tile_counts(&self) -> UVec2 {
		self.tile_counts
	}

	pub fn atlas_size(&self) -> UVec2 {
		UVec2::new(self.atlas.width(), self.atlas.height())
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
		let offset = free_tile * self.tile_size;
		imageops::replace(
			&mut self.atlas,
			sub,
			i64::from(offset.x),
			i64::from(offset.y),
		);

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
		let tex_dim: UVec2 = atlas.texture().dimensions().into();
		let texture = gfx.device.create_texture(&wgpu::TextureDescriptor {
			label,
			size: wgpu::Extent3d {
				width: tex_dim.x,
				height: tex_dim.y,
				depth_or_array_layers: 1,
			},
			mip_level_count: 1,
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
		let dim: UVec2 = atlas.texture().dimensions().into();
		debug_assert_eq!(dim, self.tex_dim);

		gfx.queue.write_texture(
			self.texture.as_image_copy(),
			bytemuck::cast_slice(atlas.texture()),
			wgpu::ImageDataLayout {
				offset: 0,
				bytes_per_row: Some(dim.x * 4 * 4),
				rows_per_image: None,
			},
			wgpu::Extent3d {
				width: atlas.texture().width(),
				height: atlas.texture().height(),
				depth_or_array_layers: 1,
			},
		)
	}
}
