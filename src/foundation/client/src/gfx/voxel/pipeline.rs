use bort::CompRef;
use crevice::std430::AsStd430;
use typed_glam::glam;
use typed_wgpu::{
	buffer::BufferBinding,
	pipeline::RenderPipeline,
	uniform::{BindGroup, BindGroupBuilder, BindGroupInstance, NoDynamicOffsets, PipelineLayout},
	vertex::{Std430VertexFormat, VertexBufferLayout},
};

use crate::engine::{
	assets::AssetManager,
	gfx::pipeline::{BindGroupExt, PipelineLayoutExt},
	io::gfx::GfxContext,
};

// === Uniforms === //

#[derive(Debug)]
pub struct VoxelRenderingBindUniform<'a> {
	pub uniforms: BufferBinding<'a, VoxelRenderingUniformBuffer>,
	pub texture: &'a wgpu::TextureView,
}

#[derive(Debug, AsStd430)]
pub struct VoxelRenderingUniformBuffer {
	pub camera: glam::Mat4,
}

impl BindGroup for VoxelRenderingBindUniform<'_> {
	type Config = ();
	type DynamicOffsets = NoDynamicOffsets;

	fn layout(builder: &mut impl BindGroupBuilder<Self>, (): &Self::Config) {
		builder
			.with_uniform_buffer(wgpu::ShaderStages::VERTEX, false, |c| {
				c.uniforms.raw.clone()
			})
			.with_texture(
				wgpu::ShaderStages::FRAGMENT,
				wgpu::TextureSampleType::Float { filterable: false },
				wgpu::TextureViewDimension::D2,
				false,
				|c| c.texture,
			);
	}
}

// === Vertices === //

#[derive(AsStd430)]
pub struct VoxelVertex {
	pub position: glam::Vec3,
	pub uv: glam::Vec2,
}

impl VoxelVertex {
	pub fn layout() -> VertexBufferLayout<Self> {
		VertexBufferLayout::builder()
			.with_attribute(Std430VertexFormat::Float32x3) // position
			.with_attribute(Std430VertexFormat::Float32x2) // uv
			.finish(wgpu::VertexStepMode::Vertex)
	}
}

// === Pipeline === //

pub fn load_opaque_block_shader(
	assets: &mut AssetManager,
	gfx: &GfxContext,
) -> CompRef<wgpu::ShaderModule> {
	assets.cache((), |_| {
		gfx.device
			.create_shader_module(wgpu::ShaderModuleDescriptor {
				label: Some("opaque_block.wgsl"),
				source: wgpu::ShaderSource::Wgsl(
					include_str!("../res/shaders/opaque_block.wgsl").into(),
				),
			})
	})
}

pub type OpaqueBlockPipeline =
	RenderPipeline<(VoxelRenderingBindUniform<'static>,), (VoxelVertex,)>;

pub fn load_opaque_block_pipeline(
	assets: &mut AssetManager,
	gfx: &GfxContext,
	surface_format: wgpu::TextureFormat,
	depth_format: wgpu::TextureFormat,
) -> CompRef<OpaqueBlockPipeline> {
	assets.cache(&(surface_format, depth_format), |assets| {
		let shader = load_opaque_block_shader(assets, gfx);

		OpaqueBlockPipeline::builder()
			.with_layout(&PipelineLayout::load_default(assets, gfx))
			.with_vertex_shader(&shader, "vs_main", &(VoxelVertex::layout(),))
			.with_fragment_shader(&shader, "fs_main", surface_format)
			.with_cull_mode(wgpu::Face::Back)
			.with_depth(depth_format, true, wgpu::CompareFunction::Less)
			.finish(&gfx.device)
	})
}

// === Uniform Management === //

#[derive(Debug)]
pub struct VoxelUniforms {
	bind_group: BindGroupInstance<VoxelRenderingBindUniform<'static>>,
	buffer: wgpu::Buffer,
}

impl VoxelUniforms {
	pub fn new(assets: &mut AssetManager, gfx: &GfxContext, texture: &wgpu::TextureView) -> Self {
		let buffer = gfx.device.create_buffer(&wgpu::BufferDescriptor {
			label: Some("uniform buffer"),
			mapped_at_creation: false,
			size: VoxelRenderingUniformBuffer::std430_size_static() as u64,
			usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
		});

		let bind_group = VoxelRenderingBindUniform {
			uniforms: buffer.as_entire_buffer_binding().into(),
			texture,
		}
		.load_instance(assets, gfx, &());

		Self { bind_group, buffer }
	}

	pub fn write_pass_state<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
		OpaqueBlockPipeline::bind_group(pass, &self.bind_group, &[]);
	}

	pub fn set_camera_matrix(&self, gfx: &GfxContext, proj: glam::Mat4) {
		gfx.queue.write_buffer(
			&self.buffer,
			0,
			VoxelRenderingUniformBuffer { camera: proj }
				.as_std430()
				.as_bytes(),
		)
	}
}
