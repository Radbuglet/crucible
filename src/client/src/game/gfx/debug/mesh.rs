use crevice::std430::AsStd430;
use crucible_common::world::math::{BlockFace, Color3, EntityAabb, EntityVec, Line3, Quad};
use crucible_util::mem::c_enum::CEnum;
use wgpu::util::DeviceExt;

use crate::engine::io::gfx::GfxContext;

use super::pipeline::{WireframePipeline, WireframeVertex};

#[derive(Debug, Default)]
pub struct DebugRenderer {
	buffer: Option<wgpu::Buffer>,
	vertices: Vec<<WireframeVertex as AsStd430>::Output>,
}

impl DebugRenderer {
	pub fn push_line(&mut self, line: Line3, color: Color3) {
		self.vertices.push(
			WireframeVertex {
				pos: line.start.to_glam().as_vec3(),
				color: color.to_glam(),
			}
			.as_std430(),
		);
		self.vertices.push(
			WireframeVertex {
				pos: line.end.to_glam().as_vec3(),
				color: color.to_glam(),
			}
			.as_std430(),
		);
	}

	pub fn push_quad(&mut self, quad: Quad<EntityVec>, color: Color3) {
		self.push_line(Line3::new(quad.0[0], quad.0[1]), color);
		self.push_line(Line3::new(quad.0[1], quad.0[2]), color);
		self.push_line(Line3::new(quad.0[2], quad.0[3]), color);
		self.push_line(Line3::new(quad.0[3], quad.0[0]), color);
	}

	pub fn push_aabb(&mut self, aabb: EntityAabb, color: Color3) {
		for face in BlockFace::variants() {
			self.push_quad(aabb.quad(face).as_quad_ccw(), color);
		}
	}

	pub fn render<'a>(
		&'a mut self,
		gfx: &GfxContext,
		pipeline: &'a WireframePipeline,
		pass: &mut wgpu::RenderPass<'a>,
	) {
		// Create vertex buffer and drop old one
		self.buffer = Some(
			gfx.device
				.create_buffer_init(&wgpu::util::BufferInitDescriptor {
					label: Some("debug wireframes"),
					usage: wgpu::BufferUsages::VERTEX,
					contents: bytemuck::cast_slice(&self.vertices),
				}),
		);

		// Bind objects
		pipeline.bind_pipeline(pass);
		WireframePipeline::bind_vertex_buffer(pass, self.buffer.as_ref().unwrap().slice(..).into());

		// Render
		pass.draw(0..self.vertices.len() as u32, 0..1);
	}
}
