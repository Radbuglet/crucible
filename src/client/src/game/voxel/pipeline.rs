use std::borrow::Cow;

use crevice::std430::AsStd430;
use crucible_common::voxel::math::{BlockFace, Sign};
use geode::prelude::*;
use typed_glam::glam;

use crate::engine::services::{gfx::GfxContext, viewport::FALLBACK_SURFACE_FORMAT};

pub struct VoxelRenderingPipeline {
	pub opaque_block_pipeline: wgpu::RenderPipeline,
	pub bind_group: wgpu::BindGroup,
	pub uniform_buffer: wgpu::Buffer,
}

impl VoxelRenderingPipeline {
	pub fn new(_s: Session, gfx: &GfxContext) -> Self {
		let opaque_block_module = gfx
			.device
			.create_shader_module(wgpu::ShaderModuleDescriptor {
				label: Some("opaque_block.wgsl"),
				source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
					"shaders/opaque_block.wgsl"
				))),
			});

		let uniform_buffer = gfx.device.create_buffer(&wgpu::BufferDescriptor {
			label: Some("uniform buffer"),
			mapped_at_creation: false,
			size: ShaderUniformBuffer::std430_size_static() as u64,
			usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
		});

		let bind_group_layout =
			gfx.device
				.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
					label: None,
					entries: &[wgpu::BindGroupLayoutEntry {
						binding: 0,
						visibility: wgpu::ShaderStages::VERTEX,
						ty: wgpu::BindingType::Buffer {
							ty: wgpu::BufferBindingType::Uniform,
							has_dynamic_offset: false,
							min_binding_size: None,
						},
						count: None,
					}],
				});

		let bind_group = gfx.device.create_bind_group(&wgpu::BindGroupDescriptor {
			label: None,
			layout: &bind_group_layout,
			entries: &[wgpu::BindGroupEntry {
				binding: 0,
				resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
					buffer: &uniform_buffer,
					offset: 0,
					size: None,
				}),
			}],
		});

		let pipeline_layout = gfx
			.device
			.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
				label: None,
				bind_group_layouts: &[&bind_group_layout],
				push_constant_ranges: &[],
			});

		let opaque_block_pipeline =
			gfx.device
				.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
					label: Some("opaque voxel pipeline"),
					layout: Some(&pipeline_layout),
					vertex: wgpu::VertexState {
						module: &opaque_block_module,
						entry_point: "vs_main",
						buffers: &[wgpu::VertexBufferLayout {
							array_stride: VoxelVertex::std430_size_static() as wgpu::BufferAddress,
							step_mode: wgpu::VertexStepMode::Vertex,
							attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3],
						}],
					},
					primitive: wgpu::PrimitiveState {
						topology: wgpu::PrimitiveTopology::TriangleList,
						strip_index_format: None,
						front_face: wgpu::FrontFace::Ccw,
						cull_mode: None,
						unclipped_depth: false,
						polygon_mode: wgpu::PolygonMode::Fill,
						conservative: false,
					},
					depth_stencil: None,
					multisample: wgpu::MultisampleState {
						count: 1,
						mask: !0,
						alpha_to_coverage_enabled: false,
					},
					fragment: Some(wgpu::FragmentState {
						module: &opaque_block_module,
						entry_point: "fs_main",
						targets: &[Some(wgpu::ColorTargetState {
							format: FALLBACK_SURFACE_FORMAT,
							blend: None,
							write_mask: wgpu::ColorWrites::all(),
						})],
					}),
					multiview: None,
				});

		Self {
			opaque_block_pipeline,
			bind_group,
			uniform_buffer,
		}
	}

	pub fn set_camera_matrix(&self, gfx: &GfxContext, proj: glam::Mat4) {
		gfx.queue.write_buffer(
			&self.uniform_buffer,
			0,
			ShaderUniformBuffer { camera: proj }.as_std430().as_bytes(),
		)
	}
}

#[derive(AsStd430)]
struct ShaderUniformBuffer {
	pub camera: glam::Mat4,
}

#[derive(AsStd430)]
pub struct VoxelVertex {
	pub position: glam::Vec3,
	pub color: glam::Vec3,
}

impl VoxelVertex {
	pub fn push_quad(
		target: &mut Vec<<Self as AsStd430>::Output>,
		mut origin: glam::Vec3,
		face: BlockFace,
	) {
		let (unit_a, unit_b) = face.ortho();
		let (unit_a, unit_b) = (
			unit_a.axis().unit().as_vec3(),
			unit_b.axis().unit().as_vec3(),
		);

		if face.sign() == Sign::Positive {
			origin += face.unit().as_vec3();
		}

		let point_a = Self {
			position: origin,
			color: glam::Vec3::new(1., 0., 0.),
		}
		.as_std430();

		let point_b = Self {
			position: origin + unit_a,
			color: glam::Vec3::new(0., 1., 0.),
		}
		.as_std430();

		let point_c = Self {
			position: origin + unit_a + unit_b,
			color: glam::Vec3::new(0., 1., 0.),
		}
		.as_std430();

		let point_d = Self {
			position: origin + unit_b,
			color: glam::Vec3::new(0., 1., 0.),
		}
		.as_std430();

		target.extend([point_a, point_b, point_c, point_a, point_c, point_d]);
	}
}
