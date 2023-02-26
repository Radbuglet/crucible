use std::borrow::Cow;

use bort::CompRef;
use crevice::std430::AsStd430;
use typed_glam::glam;

use crate::engine::{
	assets::AssetManager, gfx::texture::SamplerAssetDescriptor, io::gfx::GfxContext,
};

// === OpaqueBlockShader === //

pub fn load_opaque_block_shader(
	assets: &mut AssetManager,
	gfx: &GfxContext,
) -> CompRef<wgpu::ShaderModule> {
	assets.cache((), move |_: &mut AssetManager| {
		gfx.device
			.create_shader_module(wgpu::ShaderModuleDescriptor {
				label: Some("opaque_block.wgsl"),
				source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
					"shaders/opaque_block.wgsl"
				))),
			})
	})
}
// === VoxelPipelineLayout === //

#[derive(Debug)]
pub struct VoxelPipelineLayout {
	pub uniform_group_layout: wgpu::BindGroupLayout,
	pub pipeline_layout: wgpu::PipelineLayout,
}

pub fn load_voxel_pipeline_layout(
	assets: &mut AssetManager,
	GfxContext { device, .. }: &GfxContext,
) -> CompRef<VoxelPipelineLayout> {
	assets.cache((), move |_: &mut AssetManager| {
		let uniform_group_layout =
			device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
				label: None,
				entries: &[
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
					wgpu::BindGroupLayoutEntry {
						binding: 1,
						visibility: wgpu::ShaderStages::FRAGMENT,
						ty: wgpu::BindingType::Texture {
							sample_type: wgpu::TextureSampleType::Float { filterable: false },
							view_dimension: wgpu::TextureViewDimension::D2,
							multisampled: false,
						},
						count: None,
					},
					wgpu::BindGroupLayoutEntry {
						binding: 2,
						visibility: wgpu::ShaderStages::FRAGMENT,
						ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
						count: None,
					},
				],
			});

		let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			label: None,
			bind_group_layouts: &[&uniform_group_layout],
			push_constant_ranges: &[],
		});

		VoxelPipelineLayout {
			uniform_group_layout,
			pipeline_layout,
		}
	})
}

// === VoxelRenderingPipeline === //

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct VoxelRenderingPipelineDesc {
	pub surface_format: wgpu::TextureFormat,
	pub depth_format: wgpu::TextureFormat,
	pub is_wireframe: bool,
	pub back_face_culling: bool,
}

impl VoxelRenderingPipelineDesc {
	pub fn load(
		self,
		assets: &mut AssetManager,
		gfx: &GfxContext,
	) -> CompRef<wgpu::RenderPipeline> {
		assets.cache(self, move |assets: &mut AssetManager| {
			let shader = load_opaque_block_shader(assets, gfx);
			let layout = load_voxel_pipeline_layout(assets, gfx);

			gfx.device
				.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
					label: Some("opaque voxel pipeline"),
					layout: Some(&layout.pipeline_layout),
					vertex: wgpu::VertexState {
						module: &shader,
						entry_point: "vs_main",
						buffers: &[wgpu::VertexBufferLayout {
							array_stride: VoxelVertex::std430_size_static() as wgpu::BufferAddress,
							step_mode: wgpu::VertexStepMode::Vertex,
							attributes: &[
								wgpu::VertexAttribute {
									shader_location: 0,
									offset: 0,
									format: wgpu::VertexFormat::Float32x3,
								},
								wgpu::VertexAttribute {
									shader_location: 1,
									offset: 16,
									format: wgpu::VertexFormat::Float32x2,
								},
							],
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
				})
		})
	}
}

// // === VoxelUniforms === //

#[derive(Debug)]
pub struct VoxelUniforms {
	bind_group: wgpu::BindGroup,
	buffer: wgpu::Buffer,
}

impl VoxelUniforms {
	pub fn new(gfx: &GfxContext, assets: &mut AssetManager, texture: &wgpu::TextureView) -> Self {
		let layout = load_voxel_pipeline_layout(assets, gfx);
		let sampler = SamplerAssetDescriptor::NEAREST_CLAMP_EDGES.load(assets, gfx);

		let buffer = gfx.device.create_buffer(&wgpu::BufferDescriptor {
			label: Some("uniform buffer"),
			mapped_at_creation: false,
			size: ShaderUniformBuffer::std430_size_static() as u64,
			usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
		});

		let bind_group = gfx.device.create_bind_group(&wgpu::BindGroupDescriptor {
			label: None,
			layout: &layout.uniform_group_layout,
			entries: &[
				wgpu::BindGroupEntry {
					binding: 0,
					resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
						buffer: &buffer,
						offset: 0,
						size: None,
					}),
				},
				wgpu::BindGroupEntry {
					binding: 1,
					resource: wgpu::BindingResource::TextureView(texture),
				},
				wgpu::BindGroupEntry {
					binding: 2,
					resource: wgpu::BindingResource::Sampler(&sampler),
				},
			],
		});

		Self { bind_group, buffer }
	}

	pub fn write_pass_state<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
		pass.set_bind_group(0, &self.bind_group, &[]);
	}

	pub fn set_camera_matrix(&self, gfx: &GfxContext, proj: glam::Mat4) {
		gfx.queue.write_buffer(
			&self.buffer,
			0,
			ShaderUniformBuffer { camera: proj }.as_std430().as_bytes(),
		)
	}
}

#[derive(AsStd430)]
struct ShaderUniformBuffer {
	pub camera: glam::Mat4,
}

// === VoxelVertex === //

#[derive(AsStd430)]
pub struct VoxelVertex {
	pub position: glam::Vec3,
	pub uv: glam::Vec2,
}
