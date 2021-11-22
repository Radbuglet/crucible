use crate::engine::context::GfxContext;
use crate::engine::util::camera::GfxCameraManager;
use crate::engine::util::gpu_align_ext::convert_slice;
use crate::engine::viewport::SWAPCHAIN_FORMAT;
use cgmath::Vector3;
use crucible_core::util::meta_enum::EnumMeta;
use crucible_shared::voxel::coord::BlockFace;
use glsl_layout::{uint, vec3, Uniform};
use std::mem::size_of;
use wgpu::util::{BufferInitDescriptor, DeviceExt};

// === Internals === //

pub const DEPTH_TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

#[derive(Debug, Uniform, Copy, Clone)]
struct VoxelFaceInstance {
	pos: vec3,
	face: uint,
}

// === Rendering subsystem === //

pub struct VoxelRenderer {
	pipeline: wgpu::RenderPipeline,
	mesh: wgpu::Buffer,
}

impl VoxelRenderer {
	pub fn new(gfx: &GfxContext, camera: &GfxCameraManager) -> Self {
		// Build pipeline
		log::info!("Building voxel shading pipeline...");
		log::info!("Loading voxel vertex shader...");
		let shader_vert = gfx
			.device
			.create_shader_module(&wgpu::include_spirv!("shader/block.vert.spv"));

		log::info!("Loading voxel fragment shader...");
		let shader_frag = gfx
			.device
			.create_shader_module(&wgpu::include_spirv!("shader/block.frag.spv"));

		log::info!("Creating pipeline...");
		let pipeline_layout = gfx
			.device
			.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
				label: Some("opaque block program layout"),
				bind_group_layouts: &[camera.layout()],
				push_constant_ranges: &[],
			});

		let pipeline = gfx
			.device
			.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
				label: Some("opaque block program"),
				layout: Some(&pipeline_layout),
				vertex: wgpu::VertexState {
					module: &shader_vert,
					entry_point: "main",
					buffers: &[
						// Vertex buffer (index 0)
						wgpu::VertexBufferLayout {
							array_stride: size_of::<VoxelFaceInstance>() as wgpu::BufferAddress,
							step_mode: wgpu::VertexStepMode::Instance,
							attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Uint32],
						},
					],
				},
				primitive: wgpu::PrimitiveState {
					topology: wgpu::PrimitiveTopology::TriangleList,
					strip_index_format: None,
					front_face: wgpu::FrontFace::Ccw, // OpenGL tradition
					cull_mode: Some(wgpu::Face::Back),
					clamp_depth: false,
					polygon_mode: wgpu::PolygonMode::Fill,
					conservative: false,
				},
				depth_stencil: Some(wgpu::DepthStencilState {
					format: DEPTH_TEXTURE_FORMAT,
					depth_write_enabled: true,
					depth_compare: wgpu::CompareFunction::Less,
					stencil: Default::default(),
					bias: Default::default(),
				}),
				multisample: wgpu::MultisampleState {
					count: 1,
					mask: !0,
					alpha_to_coverage_enabled: false,
				},
				fragment: Some(wgpu::FragmentState {
					module: &shader_frag,
					entry_point: "main",
					targets: &[wgpu::ColorTargetState {
						format: SWAPCHAIN_FORMAT,
						blend: None,
						write_mask: wgpu::ColorWrites::ALL,
					}],
				}),
			});
		log::info!("Done!");

		// Generate mesh
		let mesh = {
			let mut faces = Vec::new();

			for x in 1..5 {
				for (face, _) in BlockFace::values() {
					faces.push(VoxelFaceInstance {
						pos: Vector3::new(x as f32, 0., 0.).into(),
						face: *face as u32,
					})
				}
			}

			for y in 0..5 {
				for (face, _) in BlockFace::values() {
					faces.push(VoxelFaceInstance {
						pos: Vector3::new(0., y as f32, 0.).into(),
						face: *face as u32,
					})
				}
			}

			faces
		};

		// Allocate mesh
		let mesh = gfx.device.create_buffer_init(&BufferInitDescriptor {
			label: Some("voxel mesh"),
			usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
			contents: convert_slice(&*mesh).as_slice(),
		});

		Self { pipeline, mesh }
	}

	pub fn render<'a>(&'a self, cam_group: &'a wgpu::BindGroup, pass: &mut wgpu::RenderPass<'a>) {
		pass.set_pipeline(&self.pipeline);
		pass.set_bind_group(0, cam_group, &[]);
		pass.set_vertex_buffer(0, self.mesh.slice(..));
		pass.draw(0..6, 0..(6 * 9));
	}
}
