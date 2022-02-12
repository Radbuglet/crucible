use crate::engine::context::GfxContext;
use crate::engine::util::camera::GfxCameraManager;
use crate::engine::util::contig_mesh::ContigMesh;
use crate::engine::util::std140::Std140;
use crate::engine::viewport::{DEPTH_TEXTURE_FORMAT, SWAPCHAIN_FORMAT};
use cgmath::Vector3;
use crucible_core::foundation::{Entity, Storage, World};
use crucible_core::util::format::FormatMs;
use crucible_core::util::meta_enum::EnumMeta;
use crucible_core::util::pod::{pod_struct, size_of_pod, PodWriter, VecWriter};
use crucible_shared::voxel::coord::{BlockFace, BlockPos, WorldPos};
use crucible_shared::voxel::data::VoxelWorld;
use image::EncodableLayout;
use std::time::{Duration, Instant};
use wgpu::util::DeviceExt;

// === Internals === //

pod_struct! {
	#[derive(Debug, Copy, Clone)]
	fixed struct VoxelFaceInstance {
		pos: Vector3<f32> [Std140],
		face: u32 [Std140],
	}
}

// === Rendering subsystem === //

pub struct VoxelRenderer {
	pipeline: wgpu::RenderPipeline,
	mesh: ContigMesh,
	texture_atlas: wgpu::Texture,
	texture_group: wgpu::BindGroup,
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

		let data = image::load_from_memory(include_bytes!("textures/block_texture.png"))
			.unwrap()
			.into_rgba8();

		let texture_atlas = gfx.device.create_texture_with_data(
			&gfx.queue,
			&wgpu::TextureDescriptor {
				label: Some("texture atlas"),
				size: wgpu::Extent3d {
					width: data.width(),
					height: data.height(),
					depth_or_array_layers: 1,
				},
				mip_level_count: 1,
				sample_count: 1,
				dimension: wgpu::TextureDimension::D2,
				format: wgpu::TextureFormat::Rgba8UnormSrgb,
				usage: wgpu::TextureUsages::TEXTURE_BINDING,
			},
			data.as_bytes(),
		);

		let texture_group_layout =
			gfx.device
				.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
					label: Some("opaque block uniform layout"),
					entries: &[
						wgpu::BindGroupLayoutEntry {
							ty: wgpu::BindingType::Texture {
								multisampled: false,
								sample_type: wgpu::TextureSampleType::Float { filterable: false },
								view_dimension: wgpu::TextureViewDimension::D2,
							},
							count: None,
							binding: 0,
							visibility: wgpu::ShaderStages::FRAGMENT,
						},
						wgpu::BindGroupLayoutEntry {
							ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
							count: None,
							binding: 1,
							visibility: wgpu::ShaderStages::FRAGMENT,
						},
					],
				});

		let sampler = gfx.device.create_sampler(&wgpu::SamplerDescriptor {
			address_mode_u: wgpu::AddressMode::ClampToEdge,
			address_mode_v: wgpu::AddressMode::ClampToEdge,
			address_mode_w: wgpu::AddressMode::ClampToEdge,
			mag_filter: wgpu::FilterMode::Nearest,
			min_filter: wgpu::FilterMode::Nearest,
			mipmap_filter: wgpu::FilterMode::Nearest,
			..Default::default()
		});

		let texture_group = gfx.device.create_bind_group(&wgpu::BindGroupDescriptor {
			label: Some("opaque block uniform"),
			layout: &texture_group_layout,
			entries: &[
				wgpu::BindGroupEntry {
					binding: 0,
					resource: wgpu::BindingResource::TextureView(
						&texture_atlas.create_view(&Default::default()),
					),
				},
				wgpu::BindGroupEntry {
					binding: 1,
					resource: wgpu::BindingResource::Sampler(&sampler),
				},
			],
		});

		let pipeline_layout = gfx
			.device
			.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
				label: Some("opaque block program layout"),
				bind_group_layouts: &[camera.layout(), &texture_group_layout],
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
							array_stride: size_of_pod::<VoxelFaceInstance>() as wgpu::BufferAddress,
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
			texture_atlas,
			texture_group,
			dirty: Storage::new(),
		}
	}

	pub fn mark_dirty(&mut self, world: &World, chunk: Entity) {
		self.dirty.insert(world, chunk, ());
	}

	pub async fn update_dirty(
		&mut self,
		world: &World,
		voxels: &VoxelWorld,
		_gfx: &GfxContext,
		max_duration: Duration,
	) {
		let mut mesh_data = VecWriter::new();
		let start = Instant::now();

		// Update meshes
		let mut updated = 0;
		loop {
			let dirty = match self.dirty.iter(world).next() {
				Some(mesh) => mesh.entity_id(),
				None => break,
			};

			self.dirty.remove(dirty);

			match voxels.get_chunk(world, dirty) {
				Some(chunk) => {
					// Fill up the mesh
					mesh_data.reset();
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
									mesh_data.write(&VoxelFaceInstance {
										// TODO: Improve large position handling
										pos: WorldPos::from_parts(chunk.pos(), pos)
											.raw
											.cast::<f32>()
											.unwrap(),
										face: face.marshall_shader(),
									});
								}
							}
						}
					}

					// Update mesh
					self.mesh.add(world, dirty, mesh_data.bytes()).await;
				}
				None => self.mesh.remove(world, dirty).await,
			}

			updated += 1;

			if start.elapsed() > max_duration {
				break;
			}
		}

		// Unmap buffer
		self.mesh.end_updating();
		log::info!(
			"Updated {} chunk mesh(es) in {}.",
			updated,
			FormatMs(start.elapsed())
		);
	}

	pub fn render<'a>(
		&'a self,
		// gfx: &GfxContext,
		_world: &'a World,
		cam_group: &'a wgpu::BindGroup,
		pass: &mut wgpu::RenderPass<'a>,
	) {
		pass.set_pipeline(&self.pipeline);
		pass.set_bind_group(0, cam_group, &[]);
		pass.set_bind_group(1, &self.texture_group, &[]);

		let face_count = self.mesh.len_bytes() / size_of_pod::<VoxelFaceInstance>();
		pass.set_vertex_buffer(0, self.mesh.buffer().slice(..));
		pass.draw(0..6, 0..(face_count as u32));
		log::info!(
			"Rendering {} face(s) belonging to {} chunk(s).",
			face_count,
			self.mesh.len_entries()
		);
	}
}
