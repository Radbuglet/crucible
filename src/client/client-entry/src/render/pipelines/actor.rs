use crevice::std430::AsStd430;
use crucible_assets::{Asset, AssetManager};
use main_loop::GfxContext;
use typed_glam::glam::{Mat4, Vec3};
use typed_wgpu::{
    BindGroup, BindGroupBuilder, BindGroupInstance, GpuStruct, NoDynamicOffsets, PipelineLayout,
    RenderPipeline, Std430VertexFormat, VertexBufferLayout,
};
use wgpu_ext::{BindGroupExt as _, PipelineLayoutExt as _};

// === Uniforms === //

#[derive(Debug)]
pub struct ActorBindGroup<'a> {
    pub camera: typed_wgpu::BufferBinding<'a, ActorUniformData>,
}

impl BindGroup for ActorBindGroup<'_> {
    type Config = ();
    type DynamicOffsets = NoDynamicOffsets;

    fn layout(builder: &mut impl BindGroupBuilder<Self>, _config: &Self::Config) {
        builder.with_uniform_buffer(wgpu::ShaderStages::VERTEX, false, |c| c.camera.raw.clone());
    }
}

#[derive(Debug, Copy, Clone, AsStd430)]
pub struct ActorUniformData {
    pub camera_proj: Mat4,
    pub light_proj: Mat4,
}

impl GpuStruct for ActorUniformData {
    type Pod = <Self as AsStd430>::Output;
}

// === Vertices === //

#[derive(Debug, Copy, Clone, AsStd430)]
pub struct ActorVertex {
    pub pos: Vec3,
    pub color: Vec3,
}

impl GpuStruct for ActorVertex {
    type Pod = <Self as AsStd430>::Output;
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

impl GpuStruct for ActorInstance {
    type Pod = <Self as AsStd430>::Output;
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

pub type OpaqueActorPipeline =
    RenderPipeline<(ActorBindGroup<'static>,), (ActorVertex, ActorInstance)>;

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

pub fn load_opaque_actor_shader(
    assets: &AssetManager,
    gfx: &GfxContext,
) -> Asset<wgpu::ShaderModule> {
    assets.load(gfx, (), |_assets, gfx, ()| {
        gfx.device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("actor/opaque.wgsl"),
                source: wgpu::ShaderSource::Wgsl(include_shader!("actor_opaque.wgsl").into()),
            })
    })
}

// === Uniform Management === //

#[derive(Debug)]
pub struct ActorRenderingUniforms {
    bind_group: BindGroupInstance<ActorBindGroup<'static>>,
    uniform_buffer: typed_wgpu::Buffer<ActorUniformData>,
}

impl ActorRenderingUniforms {
    pub fn new(assets: &AssetManager, gfx: &GfxContext) -> Self {
        let uniform_buffer = typed_wgpu::Buffer::create(
            &gfx.device,
            &wgpu::BufferDescriptor {
                label: Some("actor rendering uniforms buffer"),
                size: 1,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            },
        );

        let bind_group = ActorBindGroup {
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
        self.uniform_buffer.write(
            &gfx.queue,
            0,
            &[ActorUniformData {
                camera_proj,
                light_proj,
            }
            .as_std430()],
        );
    }
}