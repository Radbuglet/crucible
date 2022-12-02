use std::{borrow::Cow, sync::Arc};

use crevice::std430::AsStd430;
use crucible_common::voxel::math::{BlockFace, Sign};
use typed_glam::glam;

use crate::engine::{
	io::gfx::GfxContext,
	resources::{ResourceDescriptor, ResourceManager},
};

// === OpaqueBlockShader === //

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct OpaqueBlockShaderDesc;

impl ResourceDescriptor for OpaqueBlockShaderDesc {
	type Context<'a> = &'a GfxContext;
	type Resource = wgpu::ShaderModule;

	fn construct(
		&self,
		_res_mgr: &mut ResourceManager,
		gfx: Self::Context<'_>,
	) -> Arc<Self::Resource> {
		Arc::new(
			gfx.device
				.create_shader_module(wgpu::ShaderModuleDescriptor {
					label: Some("opaque_block.wgsl"),
					source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
						"shaders/opaque_block.wgsl"
					))),
				}),
		)
	}
}

// // === VoxelPipelineLayout === //
//
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct VoxelPipelineLayoutDesc;

#[derive(Debug)]
pub struct VoxelPipelineLayout {
	pub uniform_group_layout: wgpu::BindGroupLayout,
	pub pipeline_layout: wgpu::PipelineLayout,
}

impl ResourceDescriptor for VoxelPipelineLayoutDesc {
	type Context<'a> = &'a GfxContext;
	type Resource = VoxelPipelineLayout;

	fn construct(
		&self,
		_res_mgr: &mut ResourceManager,
		gfx: Self::Context<'_>,
	) -> Arc<Self::Resource> {
		let uniform_group_layout =
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

		let pipeline_layout = gfx
			.device
			.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
				label: None,
				bind_group_layouts: &[&uniform_group_layout],
				push_constant_ranges: &[],
			});

		Arc::new(VoxelPipelineLayout {
			uniform_group_layout,
			pipeline_layout,
		})
	}
}
//
// // === VoxelRenderingPipeline === //
//
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct VoxelRenderingPipelineDesc {
	pub surface_format: wgpu::TextureFormat,
	pub depth_format: wgpu::TextureFormat,
	pub is_wireframe: bool,
	pub back_face_culling: bool,
}

impl ResourceDescriptor for VoxelRenderingPipelineDesc {
	type Context<'a> = &'a GfxContext;
	type Resource = wgpu::RenderPipeline;

	fn construct(
		&self,
		res_mgr: &mut ResourceManager,
		gfx: Self::Context<'_>,
	) -> Arc<Self::Resource> {
		let shader = res_mgr.load(&OpaqueBlockShaderDesc, gfx);
		let layout = res_mgr.load(&VoxelPipelineLayoutDesc, gfx);

		let pipeline = gfx
			.device
			.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
				label: Some("opaque voxel pipeline"),
				layout: Some(&layout.pipeline_layout),
				vertex: wgpu::VertexState {
					module: &shader,
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
					cull_mode: if self.back_face_culling {
						Some(wgpu::Face::Back)
					} else {
						None
					},
					unclipped_depth: false,
					polygon_mode: if self.is_wireframe {
						wgpu::PolygonMode::Line
					} else {
						wgpu::PolygonMode::Fill
					},
					conservative: false,
				},
				depth_stencil: Some(wgpu::DepthStencilState {
					format: self.depth_format,
					depth_write_enabled: true,
					depth_compare: wgpu::CompareFunction::Less,
					stencil: wgpu::StencilState::default(),
					bias: wgpu::DepthBiasState::default(),
				}),
				multisample: wgpu::MultisampleState {
					count: 1,
					mask: !0,
					alpha_to_coverage_enabled: false,
				},
				fragment: Some(wgpu::FragmentState {
					module: &shader,
					entry_point: "fs_main",
					targets: &[Some(wgpu::ColorTargetState {
						format: self.surface_format,
						blend: None,
						write_mask: wgpu::ColorWrites::all(),
					})],
				}),
				multiview: None,
			});

		Arc::new(pipeline)
	}

	fn keep_alive(&self, res_mgr: &mut ResourceManager) {
		res_mgr.keep_alive(&OpaqueBlockShaderDesc);
		res_mgr.keep_alive(&VoxelPipelineLayoutDesc);
	}
}
//
// // === VoxelUniformManager === //

#[derive(Debug)]
pub struct VoxelUniforms {
	uniform_bind_group: wgpu::BindGroup,
	uniform_buffer: wgpu::Buffer,
}

impl VoxelUniforms {
	pub fn new((gfx, res_mgr): (&GfxContext, &mut ResourceManager)) -> Self {
		let layout = res_mgr.load(&VoxelPipelineLayoutDesc, gfx);

		let uniform_buffer = gfx.device.create_buffer(&wgpu::BufferDescriptor {
			label: Some("uniform buffer"),
			mapped_at_creation: false,
			size: ShaderUniformBuffer::std430_size_static() as u64,
			usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
		});

		let bind_group = gfx.device.create_bind_group(&wgpu::BindGroupDescriptor {
			label: None,
			layout: &layout.uniform_group_layout,
			entries: &[wgpu::BindGroupEntry {
				binding: 0,
				resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
					buffer: &uniform_buffer,
					offset: 0,
					size: None,
				}),
			}],
		});

		Self {
			uniform_bind_group: bind_group,
			uniform_buffer,
		}
	}

	pub fn write_pass_state<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
		pass.set_bind_group(0, &self.uniform_bind_group, &[]);
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
