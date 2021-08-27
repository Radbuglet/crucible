use crate::render::vk_prelude::*;
use cgmath::num_traits::{clamp_max, clamp_min};
use cgmath::{BaseNum, Vector2};
use winit::dpi::PhysicalSize;

// === Transformations === //

pub fn extent_to_vec2(extent: vk::Extent2D) -> Vector2<u32> {
	Vector2::new(extent.width, extent.height)
}

pub fn vec2_to_extent(vec: Vector2<u32>) -> vk::Extent2D {
	vk::Extent2D {
		width: vec.x,
		height: vec.y,
	}
}

pub fn win_sz_to_vec2(size: PhysicalSize<u32>) -> Vector2<u32> {
	Vector2::new(size.width, size.height)
}

pub fn vec2_to_win_sz(size: Vector2<u32>) -> PhysicalSize<u32> {
	PhysicalSize::new(size.x, size.y)
}

pub trait VecExt: Sized {
	fn clamp_comps(self, min: Self, max: Self) -> Self;
}

impl<T: BaseNum> VecExt for Vector2<T> {
	fn clamp_comps(self, min: Self, max: Self) -> Self {
		self.zip(min, clamp_max).zip(max, clamp_min)
	}
}
