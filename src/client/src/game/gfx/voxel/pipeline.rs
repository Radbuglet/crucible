use std::borrow::Cow;

use bort::CompRef;
use crevice::std430::AsStd430;
use typed_glam::glam;

use crate::engine::{
	assets::AssetManager,
	gfx::pipeline::{load_pipeline_layout, BindUniform, BindUniformBuilder},
	io::gfx::GfxContext,
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

// === VoxelRenderingPipeline === //

#[derive(Debug)]
pub struct VoxelRenderingUniforms<'a> {
	pub camera: wgpu::BufferBinding<'a>,
	pub texture: &'a wgpu::TextureView,
}

impl BindUniform for VoxelRenderingUniforms<'_> {
	type Config = ();

	fn layout(builder: &mut impl BindUniformBuilder<Self>, _config: Self::Config) {
		builder
			.with_uniform_buffer(wgpu::ShaderStages::VERTEX, false, |me| me.camera.clone())
			.with_texture(
				wgpu::ShaderStages::FRAGMENT,
				wgpu::TextureSampleType::Float { filterable: false },
				wgpu::TextureViewDimension::D2,
				false,
				|me| me.texture,
			);
	}
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct VoxelRenderingPipelineDesc {
	pub surface_format: wgpu::TextureFormat,
	pub depth_format: wgpu::TextureFormat,
	pub is_wireframe: bool,
	pub back_face_culling: bool,
}

impl VoxelRenderingPipelineDesc {
	pub fn load(
		&self,
		assets: &mut AssetManager,
		gfx: &GfxContext,
	) -> CompRef<wgpu::RenderPipeline> {
		assets.cache(self, move |assets: &mut AssetManager| {
			let shader = load_opaque_block_shader(assets, gfx);
			let layout = VoxelRenderingUniforms::load_layout(assets, gfx, ());
			let layout = load_pipeline_layout(assets, gfx, [&layout], []);

			gfx.device
				.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
					label: Some("opaque voxel pipeline"),
					layout: Some(&layout),
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
	pub fn new(assets: &mut AssetManager, gfx: &GfxContext, texture: &wgpu::TextureView) -> Self {
		let buffer = gfx.device.create_buffer(&wgpu::BufferDescriptor {
			label: Some("uniform buffer"),
			mapped_at_creation: false,
			size: ShaderUniformBuffer::std430_size_static() as u64,
			usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
		});

		let bind_group = VoxelRenderingUniforms {
			camera: buffer.as_entire_buffer_binding(),
			texture,
		}
		.create_instance(assets, gfx, ());

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
