use crevice::std430::AsStd430;
use crucible_assets::{Asset, AssetManager};
use main_loop::GfxContext;
use typed_glam::glam::{Mat4, Vec3};
use typed_wgpu::{
    BindGroup, BindGroupBuilder, BindGroupInstance, NoDynamicOffsets, PipelineLayout,
    RenderPipeline, Std430VertexFormat, VertexBufferLayout,
};

use crate::render::helpers::{BindGroupExt as _, PipelineLayoutExt as _};

#[derive(Debug)]
pub struct ActorRenderingBindGroup<'a> {
    pub camera: wgpu::BufferBinding<'a>,
}

impl BindGroup for ActorRenderingBindGroup<'_> {
    type Config = ();
    type DynamicOffsets = NoDynamicOffsets;

    fn layout(builder: &mut impl BindGroupBuilder<Self>, _config: &Self::Config) {
        builder.with_uniform_buffer(wgpu::ShaderStages::VERTEX, false, |c| c.camera.clone());
    }
}

#[derive(Debug, Copy, Clone, AsStd430)]
pub struct ActorRenderingUniformData {
    pub camera_proj: Mat4,
    pub light_proj: Mat4,
}

// === Vertices === //

#[derive(Debug, Copy, Clone, AsStd430)]
pub struct ActorVertex {
    pub pos: Vec3,
    pub color: Vec3,
}

impl ActorVertex {
    pub fn layout() -> VertexBufferLayout<Self> {
        VertexBufferLayout::builder()
            .with_attribute(Std430VertexFormat::Float32x3) // pos
            .with_attribute(Std430VertexFormat::Float32x3) // color
            .finish(wgpu::VertexStepMode::Vertex)
    }
}

#[derive(Debug, Copy, Clone, AsStd430)]
pub struct ActorInstance {
    pub affine_x: Vec3,
    pub affine_y: Vec3,
    pub affine_z: Vec3,
    pub translation: Vec3,
}

impl ActorInstance {
    pub fn layout() -> VertexBufferLayout<Self> {
        VertexBufferLayout::builder()
            .with_location(2)
            .with_attribute(Std430VertexFormat::Float32x3) // affine_x
            .with_attribute(Std430VertexFormat::Float32x3) // affine_y
            .with_attribute(Std430VertexFormat::Float32x3) // affine_z
            .with_attribute(Std430VertexFormat::Float32x3) // translation
            .finish(wgpu::VertexStepMode::Instance)
    }
}

// === Pipeline === //

pub fn load_opaque_actor_shader(
    assets: &AssetManager,
    gfx: &GfxContext,
) -> Asset<wgpu::ShaderModule> {
    assets.load(gfx, (), |_assets, gfx, ()| {
        gfx.device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("actor/opaque.wgsl"),
                source: wgpu::ShaderSource::Wgsl(include_str!("actor_opaque.wgsl").into()),
            })
    })
}

pub type OpaqueActorPipeline =
    RenderPipeline<(ActorRenderingBindGroup<'static>,), (ActorVertex, ActorInstance)>;

pub fn load_opaque_actor_pipeline(
    assets: &AssetManager,
    gfx: &GfxContext,
    surface_format: wgpu::TextureFormat,
    depth_format: wgpu::TextureFormat,
) -> Asset<OpaqueActorPipeline> {
    assets.load(
        gfx,
        (&surface_format, &depth_format),
        |assets, gfx, (&surface_format, &depth_format)| {
            let shader = &*load_opaque_actor_shader(assets, gfx);

            RenderPipeline::builder()
                .with_layout(&PipelineLayout::load_default(assets, gfx))
                .with_vertex_shader(
                    shader,
                    "vs_main",
                    &(ActorVertex::layout(), ActorInstance::layout()),
                )
                .with_fragment_shader(shader, "fs_main", surface_format)
                .with_cull_mode(wgpu::Face::Back)
                .with_depth(depth_format, true, wgpu::CompareFunction::Less)
                .finish(&gfx.device)
        },
    )
}

// === Uniform Management === //

#[derive(Debug)]
pub struct ActorRenderingUniforms {
    bind_group: BindGroupInstance<ActorRenderingBindGroup<'static>>,
    uniform_buffer: wgpu::Buffer,
}

impl ActorRenderingUniforms {
    pub fn new(assets: &AssetManager, gfx: &GfxContext) -> Self {
        let uniform_buffer = gfx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("actor rendering uniforms buffer"),
            size: ActorRenderingUniformData::std430_size_static() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = ActorRenderingBindGroup {
            camera: uniform_buffer.as_entire_buffer_binding(),
        }
        .load_instance(assets, gfx, ());

        Self {
            bind_group,
            uniform_buffer,
        }
    }

    pub fn write_pass_state<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        OpaqueActorPipeline::bind_group_static(pass, &self.bind_group, &[]);
    }

    pub fn set_camera_matrix(&self, gfx: &GfxContext, camera_proj: Mat4, light_proj: Mat4) {
        gfx.queue.write_buffer(
            &self.uniform_buffer,
            0,
            ActorRenderingUniformData {
                camera_proj,
                light_proj,
            }
            .as_std430()
            .as_bytes(),
        );
    }
}
