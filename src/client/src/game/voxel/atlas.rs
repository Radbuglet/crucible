use crucible_core::lang::iter::VolumetricIter;
use hashbrown::HashSet;
use image::Rgba32FImage;
use typed_glam::glam::UVec2;

pub struct AtlasBuilder {
	tile_size: UVec2,
	tile_counts: UVec2,
	free_tiles: HashSet<UVec2>,
	atlas: Rgba32FImage,
}

impl AtlasBuilder {
	pub fn new(tile_size: UVec2, tile_counts: UVec2) -> Self {
		let image_size = tile_size * tile_counts;

		Self {
			tile_size,
			tile_counts,
			free_tiles: VolumetricIter::new(tile_counts.to_array())
				.map(UVec2::from_array)
				.collect::<HashSet<_>>(),
			atlas: Rgba32FImage::new(image_size.x, image_size.y),
		}
	}

	pub fn atlas(&self) -> &Rgba32FImage {
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

	pub fn push(&mut self, sub: Rgba32FImage) -> UVec2 {
		// Allocate a free tile
		let free_tile = *self
			.free_tiles
			.iter()
			.next()
			.expect("no free tiles in atlas");

		self.free_tiles.remove(&free_tile);

		// Write to the free tile
		let offset = free_tile * self.tile_size;

		// TODO: Check performance of blitting routine
		for [x, y] in VolumetricIter::new([sub.width(), sub.height()]) {
			*self.atlas.get_pixel_mut(x + offset.x, y + offset.y) = *sub.get_pixel(x, y);
		}

		free_tile
	}
}
