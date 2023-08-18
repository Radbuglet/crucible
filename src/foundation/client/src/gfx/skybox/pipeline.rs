use bort::CompRef;
use crevice::std430::AsStd430;
use typed_glam::glam;
use typed_wgpu::{
	buffer::BufferBinding,
	pipeline::RenderPipeline,
	uniform::{BindGroup, BindGroupBuilder, BindGroupInstance, NoDynamicOffsets, PipelineLayout},
};

use crate::engine::{
	assets::AssetManager,
	gfx::pipeline::{BindGroupExt, PipelineLayoutExt},
	io::gfx::GfxContext,
};

pub fn load_skybox_shader_module(
	assets: &mut AssetManager,
	gfx: &GfxContext,
) -> CompRef<'static, wgpu::ShaderModule> {
	assets.cache((), |_assets| {
		gfx.device
			.create_shader_module(wgpu::ShaderModuleDescriptor {
				label: Some("Skybox shader module"),
				source: wgpu::ShaderSource::Wgsl(include_str!("../res/shaders/skybox.wgsl").into()),
			})
	})
}

#[derive(Debug)]
pub struct SkyboxRenderingBindUniform<'a> {
	pub uniforms: BufferBinding<'a, SkyboxRenderingUniformBuffer>,
	// pub panorama: &'a wgpu::TextureView,
}

#[derive(Debug, AsStd430)]
pub struct SkyboxRenderingUniformBuffer {
	pub inv_proj_and_view: glam::Mat4,
}

impl BindGroup for SkyboxRenderingBindUniform<'_> {
	type Config = ();
	type DynamicOffsets = NoDynamicOffsets;

	fn layout(builder: &mut impl BindGroupBuilder<Self>, (): &Self::Config) {
		builder.with_uniform_buffer(wgpu::ShaderStages::FRAGMENT, false, |c| {
			c.uniforms.raw.clone()
		});
		// .with_texture(
		// 	wgpu::ShaderStages::FRAGMENT,
		// 	wgpu::TextureSampleType::Float { filterable: false },
		// 	wgpu::TextureViewDimension::D2,
		// 	false,
		// 	|c| c.panorama,
		// );
	}
}

pub type SkyboxPipeline = RenderPipeline<(SkyboxRenderingBindUniform<'static>,), ()>;

pub fn load_skybox_pipeline(
	assets: &mut AssetManager,
	gfx: &GfxContext,
	surface_format: wgpu::TextureFormat,
) -> CompRef<'static, SkyboxPipeline> {
	assets.cache(&surface_format, |assets| {
		let shader = load_skybox_shader_module(assets, gfx);

		SkyboxPipeline::builder()
			.with_layout(&PipelineLayout::load_default(assets, gfx))
			.with_vertex_shader(&shader, "vs_main", &())
			.with_fragment_shader(&shader, "fs_main", surface_format)
			.finish(&gfx.device)
	})
}

#[derive(Debug)]
pub struct SkyboxUniforms {
	bind_group: BindGroupInstance<SkyboxRenderingBindUniform<'static>>,
	buffer: wgpu::Buffer,
}

impl SkyboxUniforms {
	pub fn new(
		assets: &mut AssetManager,
		gfx: &GfxContext,
		/* panorama: &wgpu::TextureView */
	) -> Self {
		let buffer = gfx.device.create_buffer(&wgpu::BufferDescriptor {
			label: Some("skybox uniform buffer"),
			mapped_at_creation: false,
			size: SkyboxRenderingUniformBuffer::std430_size_static() as u64,
			usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
		});

		let bind_group = SkyboxRenderingBindUniform {
			uniforms: buffer.as_entire_buffer_binding().into(),
			// panorama,
		}
		.load_instance(assets, gfx, &());

		Self { bind_group, buffer }
	}

	pub fn write_pass_state<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
		SkyboxPipeline::bind_group(pass, &self.bind_group, &[]);
	}

	pub fn set_camera_matrix(&self, gfx: &GfxContext, inv_proj_and_view: glam::Mat4) {
		gfx.queue.write_buffer(
			&self.buffer,
			0,
			SkyboxRenderingUniformBuffer { inv_proj_and_view }
				.as_std430()
				.as_bytes(),
		)
	}
}
