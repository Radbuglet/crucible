use std::f32::consts::TAU;

use crevice::std430::AsStd430;
use crucible_foundation_shared::math::Color4;
use typed_glam::glam::Vec2;
use typed_wgpu::vertex::{Std430VertexFormat, VertexBufferLayout};

// === Vertices === //

#[derive(AsStd430)]
pub struct MeshInstance {
	pub pos: u32,
	pub size: u32,
	pub angle_depth: u32,
	pub color: u32,
}

impl MeshInstance {
	pub fn new(pos: Vec2, size: Vec2, angle: f32, depth: u16, color: Color4) -> Self {
		let pos_x = (pos.x * u16::MAX as f32) as u16 as u32;
		let pos_y = (pos.y * u16::MAX as f32) as u16 as u32;
		let size_x = (size.x * u16::MAX as f32) as u16 as u32;
		let size_y = (size.y * u16::MAX as f32) as u16 as u32;
		let color_r = (color.x() * u8::MAX as f32) as u8 as u32;
		let color_g = (color.y() * u8::MAX as f32) as u8 as u32;
		let color_b = (color.z() * u8::MAX as f32) as u8 as u32;
		let color_a = (color.w() * u8::MAX as f32) as u8 as u32;

		let angle = (angle / TAU * u16::MAX as f32) as u16 as u32;
		let depth = depth as u32;

		let pos = pos_x << 16 + pos_y;
		let size = size_x << 16 + size_y;
		let angle_depth = angle << 16 + depth;
		let color = {
			let mut accum = 0;
			accum += color_r;
			accum <<= 8;
			accum += color_g;
			accum <<= 8;
			accum += color_b;
			accum <<= 8;
			accum += color_a;
			accum
		};

		Self {
			pos,
			size,
			angle_depth,
			color,
		}
	}

	pub fn layout() -> VertexBufferLayout<Self> {
		VertexBufferLayout::builder()
			.with_attribute(Std430VertexFormat::Uint32) // pos
			.with_attribute(Std430VertexFormat::Uint32) // size
			.with_attribute(Std430VertexFormat::Float32) // angle_depth
			.with_attribute(Std430VertexFormat::Uint32) // color
			.finish(wgpu::VertexStepMode::Instance)
	}
}

#[derive(AsStd430)]
pub struct MeshVertex {
	pub pos: Vec2,
	pub uv: Vec2,
}

impl MeshVertex {
	pub fn layout() -> VertexBufferLayout<Self> {
		VertexBufferLayout::builder()
			.with_attribute(Std430VertexFormat::Float32x2) // pos
			.with_attribute(Std430VertexFormat::Float32x2) // uv
			.finish(wgpu::VertexStepMode::Vertex)
	}
}
