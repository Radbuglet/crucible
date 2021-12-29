use crate::engine::context::GfxContext;
use crate::engine::util::camera::GfxCameraManager;
use crate::engine::util::gpu_align_ext::convert_slice;
use crate::engine::viewport::SWAPCHAIN_FORMAT;
use crate::voxel::data::VoxelWorld;
use crucible_core::foundation::{Entity, Storage, World};
use crucible_core::util::meta_enum::EnumMeta;
use crucible_shared::voxel::coord::{BlockFace, WorldPos};
use glsl_layout::{uint, vec3, Uniform};
use std::collections::VecDeque;
use std::mem::size_of;
use std::time::{Duration, Instant};
use wgpu::util::DeviceExt;

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
	meshes: Storage<ChunkEntry>,
	dirty: VecDeque<Entity>,
}

#[derive(Debug)]
struct ChunkEntry {
	buffer: wgpu::Buffer,
	count: u32,
	dirty: bool,
}

impl VoxelRenderer {
	pub fn new(gfx: &GfxContext, camera: &GfxCameraManager) -> Self {
		let shader_vert = gfx
			.device
			.create_shader_module(&wgpu::include_spirv!("shader/block.vert.spv"));

		let shader_frag = gfx
			.device
			.create_shader_module(&wgpu::include_spirv!("shader/block.frag.spv"));

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

		Self {
			pipeline,
			meshes: Storage::new(),
			dirty: VecDeque::new(),
		}
	}

	pub fn mark_dirty(&mut self, world: &World, chunk: Entity) {
		if let Some(mesh) = self.meshes.try_get_mut(world, chunk) {
			mesh.dirty = true;
		}

		self.dirty.push_back(chunk);
	}

	pub fn update_dirty(
		&mut self,
		world: &World,
		voxels: &VoxelWorld,
		gfx: &GfxContext,
		max_duration: Duration,
	) {
		let mut mesh_faces = Vec::new();
		let start = Instant::now();

		loop {
			let dirty = match self.dirty.pop_front() {
				Some(dirty) => dirty,
				None => break,
			};

			match voxels.get_chunk(world, dirty) {
				Some(chunk) => {
					// Ensure we're not re-meshing an already-updated chunk.
					let mesh = self.meshes.try_get_mut(world, dirty);
					match mesh {
						Some(ChunkEntry { dirty: false, .. }) => continue,
						_ => {}
					}

					// Fill up the mesh
					mesh_faces.clear();
					for (pos, block) in chunk.blocks() {
						if block != 0 {
							for face in BlockFace::variants() {
								mesh_faces.push(VoxelFaceInstance {
									pos: WorldPos::from_parts(chunk.pos(), pos)
										.raw
										.cast::<f32>()
										.unwrap()
										.into(),
									face: face.marshall_shader(),
								});
							}
						}
					}

					// Upload mesh to buffer
					match mesh {
						Some(entry) => {
							gfx.queue.write_buffer(
								&entry.buffer,
								0,
								convert_slice(&mesh_faces).as_slice(),
							);
							entry.count = mesh_faces.len() as u32;
							entry.dirty = false;
						}
						None => {
							let buffer =
								gfx.device
									.create_buffer_init(&wgpu::util::BufferInitDescriptor {
										label: Some(format!("Chunk {:?} mesh", dirty).as_str()),
										usage: wgpu::BufferUsages::VERTEX
											| wgpu::BufferUsages::COPY_DST,
										contents: convert_slice(&mesh_faces).as_slice(),
									});

							self.meshes.insert(
								world,
								dirty,
								ChunkEntry {
									buffer,
									count: mesh_faces.len() as u32,
									dirty: false,
								},
							);
						}
					}
				}
				None => {
					self.meshes.remove(dirty);
				}
			}

			if start.elapsed() > max_duration {
				break;
			}
		}
	}

	pub fn render<'a>(
		&'a self,
		world: &'a World,
		cam_group: &'a wgpu::BindGroup,
		pass: &mut wgpu::RenderPass<'a>,
	) {
		pass.set_pipeline(&self.pipeline);
		pass.set_bind_group(0, cam_group, &[]);

		for (_, mesh) in self.meshes.iter(world) {
			pass.set_vertex_buffer(0, mesh.buffer.slice(..));
			pass.draw(0..6, 0..mesh.count);
		}
	}
}
