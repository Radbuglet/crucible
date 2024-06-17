use crevice::std430::AsStd430;
use crucible_assets::{Asset, AssetManager};
use main_loop::GfxContext;
use typed_glam::glam;
use typed_wgpu::{
    BindGroup, BindGroupBuilder, BindGroupInstance, BufferBinding, NoDynamicOffsets,
    PipelineLayout, RenderPipeline,
};

use crate::render::helpers::{BindGroupExt as _, PipelineLayoutExt as _, SamplerDesc};

pub fn load_skybox_shader_module(
    assets: &AssetManager,
    gfx: &GfxContext,
) -> Asset<wgpu::ShaderModule> {
    assets.load(gfx, (), |_assets, gfx, ()| {
        gfx.device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Skybox shader module"),
                source: wgpu::ShaderSource::Wgsl(include_str!("skybox.wgsl").into()),
            })
    })
}

#[derive(Debug)]
pub struct SkyboxRenderingBindUniform<'a> {
    pub uniforms: BufferBinding<'a, SkyboxRenderingUniformBuffer>,
    pub panorama: &'a wgpu::TextureView,
    pub panorama_sampler: &'a wgpu::Sampler,
}

#[derive(Debug, AsStd430)]
pub struct SkyboxRenderingUniformBuffer {
    pub inv_proj_and_view: glam::Mat4,
}

impl BindGroup for SkyboxRenderingBindUniform<'_> {
    type Config = ();
    type DynamicOffsets = NoDynamicOffsets;

    fn layout(builder: &mut impl BindGroupBuilder<Self>, (): &Self::Config) {
        builder
            .with_uniform_buffer(wgpu::ShaderStages::FRAGMENT, false, |c| {
                c.uniforms.raw.clone()
            })
            .with_texture(
                wgpu::ShaderStages::FRAGMENT,
                wgpu::TextureSampleType::Float { filterable: true },
                wgpu::TextureViewDimension::D2,
                false,
                |c| c.panorama,
            )
            .with_sampler(
                wgpu::ShaderStages::FRAGMENT,
                wgpu::SamplerBindingType::Filtering,
                |c| c.panorama_sampler,
            );
    }
}

pub type SkyboxPipeline = RenderPipeline<(SkyboxRenderingBindUniform<'static>,), ()>;

pub fn load_skybox_pipeline(
    assets: &AssetManager,
    gfx: &GfxContext,
    surface_format: wgpu::TextureFormat,
) -> Asset<SkyboxPipeline> {
    assets.load(gfx, (&surface_format,), |assets, gfx, (surface_format,)| {
        let shader = load_skybox_shader_module(assets, gfx);

        SkyboxPipeline::builder()
            .with_layout(&PipelineLayout::load_default(assets, gfx))
            .with_vertex_shader(&shader, "vs_main", &())
            .with_fragment_shader(&shader, "fs_main", *surface_format)
            .finish(&gfx.device)
    })
}

#[derive(Debug)]
pub struct SkyboxUniforms {
    bind_group: BindGroupInstance<SkyboxRenderingBindUniform<'static>>,
    buffer: wgpu::Buffer,
}

impl SkyboxUniforms {
    pub fn new(assets: &AssetManager, gfx: &GfxContext, panorama: &wgpu::TextureView) -> Self {
        let buffer = gfx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("skybox uniform buffer"),
            mapped_at_creation: false,
            size: SkyboxRenderingUniformBuffer::std430_size_static() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group = SkyboxRenderingBindUniform {
            uniforms: BufferBinding::wrap(buffer.as_entire_buffer_binding()),
            panorama,
            panorama_sampler: &SamplerDesc::FILTER_CLAMP_EDGES.load(assets, gfx),
        }
        .load_instance(assets, gfx, ());

        Self { bind_group, buffer }
    }

    pub fn write_pass_state<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        SkyboxPipeline::bind_group_static(pass, &self.bind_group, &[]);
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
