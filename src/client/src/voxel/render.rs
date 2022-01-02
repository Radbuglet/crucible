use crate::engine::context::GfxContext;
use crate::engine::util::camera::GfxCameraManager;
use crate::engine::util::contig_mesh::ContigMesh;
use crate::engine::util::gpu_align_ext::convert_slice;
use crate::engine::viewport::SWAPCHAIN_FORMAT;
use crucible_core::foundation::{Entity, Storage, World};
use crucible_core::util::meta_enum::EnumMeta;
use crucible_shared::voxel::coord::{BlockFace, BlockPos, WorldPos};
use crucible_shared::voxel::data::VoxelWorld;
use futures::executor::block_on;
use glsl_layout::{uint, vec3, Uniform};
use std::mem::size_of;
use std::time::{Duration, Instant};

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
	pub mesh: ContigMesh,
	dirty: Storage<()>,
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
					unclipped_depth: false,
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
				multiview: None,
			});

		let mesh = ContigMesh::new(gfx);

		Self {
			pipeline,
			mesh,
			dirty: Storage::new(),
		}
	}

	pub fn mark_dirty(&mut self, world: &World, chunk: Entity) {
		self.dirty.insert(world, chunk, ());
	}

	pub fn update_dirty(
		&mut self,
		world: &World,
		voxels: &VoxelWorld,
		_gfx: &GfxContext,
		max_duration: Duration,
	) {
		let mut mesh_faces = Vec::new();
		let start = Instant::now();

		// Map buffer
		block_on(self.mesh.begin_updating()).unwrap();

		// Update meshes
		loop {
			let dirty = match self.dirty.iter(world).next() {
				Some((entity, _)) => entity,
				None => break,
			};

			self.dirty.remove(dirty);

			match voxels.get_chunk(world, dirty) {
				Some(chunk) => {
					// Fill up the mesh
					mesh_faces.clear();
					for (pos, block) in chunk.blocks() {
						if block != 0 {
							for face in BlockFace::variants() {
								let neighbor_pos = (pos.raw.cast::<i8>().unwrap()
									+ face.unit::<i8>())
								.cast::<u8>();

								let make_face = match neighbor_pos {
									Some(pos) => {
										let pos = BlockPos::from(pos);
										!pos.is_valid() || chunk.get_block(pos) == 0
									}
									_ => true,
								};

								if make_face {
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
					}

					// Update mesh
					self.mesh
						.add(world, dirty, convert_slice(&*mesh_faces).as_slice());
				}
				None => self.mesh.remove(world, dirty),
			}

			if start.elapsed() > max_duration {
				break;
			}
		}

		// Unmap buffer
		self.mesh.end_updating();
	}

	pub fn render<'a>(
		&'a self,
		_world: &'a World,
		cam_group: &'a wgpu::BindGroup,
		pass: &mut wgpu::RenderPass<'a>,
	) {
		pass.set_pipeline(&self.pipeline);
		pass.set_bind_group(0, cam_group, &[]);

		let face_count =
			self.mesh.len_bytes() / std::mem::size_of::<<VoxelFaceInstance as Uniform>::Std140>();

		pass.set_vertex_buffer(0, self.mesh.buffer().slice(..));
		pass.draw(0..6, 0..(face_count as u32));
		log::info!(
			"Rendering {} face(s) belonging to {} chunk(s).",
			face_count,
			self.mesh.len_entries()
		);
	}
}
