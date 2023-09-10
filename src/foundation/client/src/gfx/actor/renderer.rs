use crevice::std430::AsStd430;
use crucible_foundation_shared::{
	math::{Color3, Tri},
	mesh::QuadMeshLayer,
};
use crucible_util::mem::array::map_arr;
use typed_glam::glam::Affine3A;
use typed_wgpu::buffer::BufferSlice;

use crate::engine::{
	gfx::buffer::{buffer_len_to_count, DynamicBuffer},
	io::gfx::GfxContext,
};

use super::pipeline::{ActorInstance, ActorVertex, OpaqueActorPipeline};

pub type ActorMeshLayer = QuadMeshLayer<Color3>;

#[derive(Debug)]
pub struct ActorRenderer {
	vertex_buffer: DynamicBuffer,
	instance_buffer: DynamicBuffer,
	mesh_endings: Vec<MeshEnding>,
}

#[derive(Debug, Copy, Clone)]
struct MeshEnding {
	last_vertex: u32,
	last_instance: u32,
}

impl Default for ActorRenderer {
	fn default() -> Self {
		Self {
			vertex_buffer: DynamicBuffer::new(
				"actor vertex buffer",
				wgpu::BufferUsages::VERTEX,
				1024,
			),
			instance_buffer: DynamicBuffer::new(
				"actor instance buffer",
				wgpu::BufferUsages::VERTEX,
				1024,
			),
			mesh_endings: Default::default(),
		}
	}
}

impl ActorRenderer {
	pub fn push_model(&mut self, gfx: &GfxContext, model: &ActorMeshLayer) {
		for (quad, color) in model.iter_cloned() {
			let [Tri([a, b, c]), Tri([d, e, f])] = quad.as_quad_ccw().to_tris();
			let quad_vertices = [a, b, c, d, e, f];
			let quad_vertices = map_arr(quad_vertices, |pos| {
				ActorVertex {
					pos,
					color: color.to_glam(),
				}
				.as_std430()
			});

			self.vertex_buffer
				.push(gfx, bytemuck::cast_slice(&quad_vertices));
		}

		self.mesh_endings.push(MeshEnding {
			last_vertex: buffer_len_to_count::<ActorVertex>(self.vertex_buffer.len()),
			last_instance: buffer_len_to_count::<ActorInstance>(self.instance_buffer.len()),
		});
	}

	pub fn push_model_instance(&mut self, gfx: &GfxContext, affine: Affine3A) {
		self.instance_buffer.push(
			gfx,
			ActorInstance {
				affine_x: affine.x_axis.into(),
				affine_y: affine.y_axis.into(),
				affine_z: affine.z_axis.into(),
				translation: affine.translation.into(),
			}
			.as_std430()
			.as_bytes(),
		);
		self.mesh_endings.last_mut().unwrap().last_instance =
			buffer_len_to_count::<ActorInstance>(self.instance_buffer.len());
	}

	pub fn upload(&mut self, gfx: &GfxContext, cb: &mut wgpu::CommandEncoder) {
		self.instance_buffer.upload(gfx, cb);
		self.vertex_buffer.upload(gfx, cb);
	}

	pub fn render<'a>(&'a mut self, pass: &mut wgpu::RenderPass<'a>) {
		OpaqueActorPipeline::bind_vertex_buffer(
			pass,
			BufferSlice::<ActorInstance>::wrap(self.instance_buffer.buffer().slice(..)),
		);

		OpaqueActorPipeline::bind_vertex_buffer(
			pass,
			BufferSlice::<ActorVertex>::wrap(self.vertex_buffer.buffer().slice(..)),
		);

		let mut last_instance_end = 0;
		let mut last_vertex_end = 0;

		for &ending in &self.mesh_endings {
			pass.draw(
				last_vertex_end..ending.last_vertex,
				last_instance_end..ending.last_instance,
			);
			last_instance_end = ending.last_instance;
			last_vertex_end = ending.last_vertex;
		}
	}

	pub fn reset_and_release(&mut self) {
		self.instance_buffer.reset_and_release();
		self.vertex_buffer.reset_and_release();
		self.mesh_endings.clear();
	}
}
