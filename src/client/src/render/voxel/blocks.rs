// TODO: This can all get cleaned up once we start working on some actual framework-y rendering stuff.
// ^ for example, we could use OpenGL-style uniform handling: https://stackoverflow.com/questions/54103399/how-to-repeatedly-update-a-uniform-data-for-number-of-objects-inside-a-single-vu

use crate::render::core::context::GfxContext;
use crate::render::core::viewport::SWAPCHAIN_FORMAT;
use crate::util::pod_ext::{Mat4PodAdapter, Vec3PodAdapter};
use bytemuck::{bytes_of, Pod, Zeroable};
use cgmath::{Deg, Matrix4, Transform, Vector3};
use crucible_core::util::error::AnyResult;
use crucible_core::util::meta_enum::EnumMeta;
use crucible_shared::voxel::coord::BlockFace;
use std::mem::size_of;
use std::time::{Duration, Instant};
use wgpu::util::{BufferInitDescriptor, DeviceExt};

// === Internals === //

fn get_proj_matrix(time: Duration) -> Matrix4<f32> {
	let proj_matrix = cgmath::perspective(Deg(90.0), 1., 0.01, 500.0);
	let world_matrix =
		Matrix4::from_translation(Vector3::new(-10., time.as_secs_f32().cos() * 5., 10.))
			* Matrix4::from_angle_y(Deg(-45.))
			* Matrix4::from_angle_x(Deg(0.));

	proj_matrix * world_matrix.inverse_transform().unwrap()
}

#[derive(Debug, Pod, Zeroable, Copy, Clone)]
#[repr(C)]
struct CamBuffer {
	proj: Mat4PodAdapter<f32>,
}

#[derive(Debug, Pod, Zeroable, Copy, Clone)]
#[repr(C)]
struct MeshVertex {
	pos: Vec3PodAdapter<f32>,
	// N.B. This doesn't waste memory because the type must have no padding anyways.
	material: u32,
}

#[derive(Default)]
struct MeshBuilder {
	vertices: Vec<MeshVertex>,
}

impl MeshBuilder {
	pub fn new() -> Self {
		Default::default()
	}

	pub fn push_quad(&mut self, vertices: [MeshVertex; 4]) {
		// Quad layout:
		// 1---2
		// |   |
		// 0---3

		// Tri 1 (0, 1, 2)
		self.vertices.push(vertices[0]);
		self.vertices.push(vertices[1]);
		self.vertices.push(vertices[2]);

		// Tri 2 (0, 2, 3)
		self.vertices.push(vertices[0]);
		self.vertices.push(vertices[2]);
		self.vertices.push(vertices[3]);
	}

	pub fn as_bytes(&self) -> &[u8] {
		bytemuck::cast_slice(self.vertices.as_slice())
	}
}

// === Rendering subsystem === //

pub struct VoxelRenderer {
	pipeline: wgpu::RenderPipeline,
	cam_group: wgpu::BindGroup,
	cam_buffer: wgpu::Buffer,
	mesh: wgpu::Buffer,
	start: Instant,
}

impl VoxelRenderer {
	pub fn new(gfx: &GfxContext) -> AnyResult<Self> {
		// Build pipeline
		log::info!("Building voxel shading pipeline...");
		log::info!("Loading voxel vertex shader...");
		let shader_vert = gfx
			.device
			.create_shader_module(&wgpu::include_spirv!("../shader/triangle.vert.spv"));

		log::info!("Loading voxel fragment shader...");
		let shader_frag = gfx
			.device
			.create_shader_module(&wgpu::include_spirv!("../shader/triangle.frag.spv"));

		log::info!("Creating pipeline...");
		let cam_group_layout =
			gfx.device
				.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
					label: Some("cam group layout"),
					entries: &[
						// Primary block
						wgpu::BindGroupLayoutEntry {
							binding: 0,
							visibility: wgpu::ShaderStages::VERTEX,
							ty: wgpu::BindingType::Buffer {
								ty: wgpu::BufferBindingType::Uniform,
								has_dynamic_offset: false,
								min_binding_size: None,
							},
							count: None,
						},
					],
				});

		let cam_buffer = gfx.device.create_buffer_init(&BufferInitDescriptor {
			label: Some("camera uniform buffer"),
			usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
			contents: bytes_of(&CamBuffer {
				proj: Mat4PodAdapter(get_proj_matrix(Duration::ZERO)),
			}),
		});

		let cam_group = gfx.device.create_bind_group(&wgpu::BindGroupDescriptor {
			label: Some("camera uniform group"),
			layout: &cam_group_layout,
			entries: &[wgpu::BindGroupEntry {
				binding: 0,
				resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
					buffer: &cam_buffer,
					offset: 0,
					size: None,
				}),
			}],
		});

		let pipeline_layout = gfx
			.device
			.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
				label: Some("opaque block program layout"),
				bind_group_layouts: &[&cam_group_layout],
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
							array_stride: size_of::<MeshVertex>() as wgpu::BufferAddress,
							step_mode: wgpu::VertexStepMode::Vertex,
							attributes: &[
								// a_pos
								wgpu::VertexAttribute {
									offset: 0,
									format: wgpu::VertexFormat::Float32x3,
									shader_location: 0,
								},
								// a_mat
								wgpu::VertexAttribute {
									offset: size_of::<Vector3<f32>>() as wgpu::BufferAddress,
									format: wgpu::VertexFormat::Uint32,
									shader_location: 3,
								},
							],
						},
					],
				},
				primitive: wgpu::PrimitiveState {
					topology: wgpu::PrimitiveTopology::TriangleList,
					strip_index_format: None,
					front_face: wgpu::FrontFace::Ccw, // OpenGL tradition
					cull_mode: None,
					clamp_depth: false,
					polygon_mode: wgpu::PolygonMode::Line,
					conservative: false,
				},
				depth_stencil: None,
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
			let mut builder = MeshBuilder::new();

			for (face, _) in BlockFace::values() {
				let material = rand::random::<u32>();
				let vertices = face.quad_ccw();

				builder.push_quad([
					MeshVertex {
						pos: Vec3PodAdapter(vertices[0]),
						material,
					},
					MeshVertex {
						pos: Vec3PodAdapter(vertices[1]),
						material,
					},
					MeshVertex {
						pos: Vec3PodAdapter(vertices[2]),
						material,
					},
					MeshVertex {
						pos: Vec3PodAdapter(vertices[3]),
						material,
					},
				]);
			}

			builder
		};

		// Allocate mesh
		let mesh = gfx.device.create_buffer_init(&BufferInitDescriptor {
			label: Some("voxel mesh"),
			usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
			contents: mesh.as_bytes(),
		});

		Ok(Self {
			pipeline,
			cam_group,
			cam_buffer,
			mesh,
			start: Instant::now(),
		})
	}

	pub fn render<'a>(&'a self, gfx: &GfxContext, pass: &mut wgpu::RenderPass<'a>) {
		// Update projection matrix
		// TODO: This isn't idiomatic or even correct but we'll just keep using this until we have proper rendering infrastructure.
		gfx.queue.write_buffer(
			&self.cam_buffer,
			0,
			bytes_of(&CamBuffer {
				proj: Mat4PodAdapter(get_proj_matrix(self.start.elapsed())),
			}),
		);

		// Render
		pass.set_pipeline(&self.pipeline);
		pass.set_bind_group(0, &self.cam_group, &[]);
		pass.set_vertex_buffer(0, self.mesh.slice(..));
		pass.draw(0..36, 0..1);
	}
}
