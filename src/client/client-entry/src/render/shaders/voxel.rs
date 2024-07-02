use crevice::std430::AsStd430;
use crucible_assets::{Asset, AssetManager};
use main_loop::GfxContext;
use typed_glam::glam;
use typed_wgpu::{
    BindGroup, BindGroupBuilder, BindGroupInstance, BufferBinding, GpuStruct, NoDynamicOffsets,
    PipelineLayout, RenderPipeline, Std430VertexFormat, VertexBufferLayout,
};

use crate::render::helpers::{BindGroupExt as _, PipelineLayoutExt, SamplerDesc};

// === Uniforms === //

#[derive(Debug)]
pub struct VoxelCommonBindGroup<'a> {
    pub uniforms: BufferBinding<'a, VoxelCommonUniformData>,
    pub texture: &'a wgpu::TextureView,
    pub nearest_sampler: &'a wgpu::Sampler,
}

#[derive(Debug, AsStd430)]
pub struct VoxelCommonUniformData {
    pub camera: glam::Mat4,
    pub light: glam::Mat4,
}

impl GpuStruct for VoxelCommonUniformData {
    type Pod = <Self as AsStd430>::Output;
}

impl BindGroup for VoxelCommonBindGroup<'_> {
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
            )
            .with_sampler(
                wgpu::ShaderStages::FRAGMENT,
                wgpu::SamplerBindingType::NonFiltering,
                |c| c.nearest_sampler,
            );
    }
}

#[derive(Debug)]
pub struct VoxelOpaqueBindGroup<'a> {
    pub depth_texture: &'a wgpu::TextureView,
}

impl BindGroup for VoxelOpaqueBindGroup<'_> {
    type Config = ();
    type DynamicOffsets = NoDynamicOffsets;

    fn layout(builder: &mut impl BindGroupBuilder<Self>, (): &Self::Config) {
        builder.with_texture(
            wgpu::ShaderStages::FRAGMENT,
            wgpu::TextureSampleType::Float { filterable: false },
            wgpu::TextureViewDimension::D2,
            false,
            |c| c.depth_texture,
        );
    }
}

// === Vertices === //

#[derive(AsStd430)]
pub struct VoxelVertex {
    pub position: glam::Vec3,
    pub uv: glam::Vec2,
}

impl GpuStruct for VoxelVertex {
    type Pod = <Self as AsStd430>::Output;
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

pub type VoxelOpaquePipeline =
    RenderPipeline<(VoxelCommonBindGroup<'static>, VoxelOpaqueBindGroup<'static>), (VoxelVertex,)>;

pub fn load_voxel_opaque_pipeline(
    assets: &AssetManager,
    gfx: &GfxContext,
    surface_format: wgpu::TextureFormat,
    depth_format: wgpu::TextureFormat,
) -> Asset<VoxelOpaquePipeline> {
    assets.load(
        gfx,
        (&surface_format, &depth_format),
        |assets, gfx, (&surface_format, &depth_format)| {
            let shader = load_voxel_opaque_shader(assets, gfx);

            VoxelOpaquePipeline::builder()
                .with_layout(&PipelineLayout::load_default(assets, gfx))
                .with_vertex_shader(&shader, "vs_main", &(VoxelVertex::layout(),))
                .with_fragment_shader(&shader, "fs_main", surface_format)
                .with_cull_mode(wgpu::Face::Back)
                .with_depth(depth_format, true, wgpu::CompareFunction::Less)
                .finish(&gfx.device)
        },
    )
}

pub fn load_voxel_opaque_shader(
    assets: &AssetManager,
    gfx: &GfxContext,
) -> Asset<wgpu::ShaderModule> {
    assets.load(gfx, (), |_, gfx, ()| {
        gfx.device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("voxel_opaque.wgsl"),
                source: wgpu::ShaderSource::Wgsl(include_str!("voxel_opaque.wgsl").into()),
            })
    })
}

pub type VoxelCsmPipeline = RenderPipeline<(VoxelCommonBindGroup<'static>,), (VoxelVertex,)>;

pub fn load_voxel_csm_pipeline(
    assets: &AssetManager,
    gfx: &GfxContext,
    depth_format: wgpu::TextureFormat,
) -> Asset<VoxelCsmPipeline> {
    assets.load(gfx, (&depth_format,), |assets, gfx, (&depth_format,)| {
        let shader = load_voxel_csm_shader(assets, gfx);

        VoxelCsmPipeline::builder()
            .with_layout(&PipelineLayout::load_default(assets, gfx))
            .with_vertex_shader(&shader, "vs_main", &(VoxelVertex::layout(),))
            .with_cull_mode(wgpu::Face::Back)
            .with_depth(depth_format, true, wgpu::CompareFunction::Less)
            .finish(&gfx.device)
    })
}

pub fn load_voxel_csm_shader(assets: &AssetManager, gfx: &GfxContext) -> Asset<wgpu::ShaderModule> {
    assets.load(gfx, (), |_, gfx, ()| {
        gfx.device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("voxel_csm.wgsl"),
                source: wgpu::ShaderSource::Wgsl(include_str!("voxel_csm.wgsl").into()),
            })
    })
}

// === Uniform Management === //

#[derive(Debug)]
pub struct VoxelUniforms {
    buffer: typed_wgpu::Buffer<VoxelCommonUniformData>,
    common_bind_group: BindGroupInstance<VoxelCommonBindGroup<'static>>,
    opaque_bind_group: BindGroupInstance<VoxelOpaqueBindGroup<'static>>,
}

impl VoxelUniforms {
    pub fn new(
        assets: &AssetManager,
        gfx: &GfxContext,
        texture: &wgpu::TextureView,
        depth_texture: &wgpu::TextureView,
    ) -> Self {
        let buffer = typed_wgpu::Buffer::create(
            &gfx.device,
            &wgpu::BufferDescriptor {
                label: Some("uniform buffer"),
                mapped_at_creation: false,
                size: 1,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            },
        );

        // Create `common_bind_group`
        let nearest_sampler = SamplerDesc {
            mipmap_filter: wgpu::FilterMode::Linear,
            lod_min_clamp: 0.0,
            lod_max_clamp: 4.0,
            ..SamplerDesc::NEAREST_CLAMP_EDGES
        }
        .load(assets, gfx);

        let common_bind_group = VoxelCommonBindGroup {
            uniforms: buffer.as_entire_buffer_binding(),
            texture,
            nearest_sampler: &nearest_sampler,
        }
        .load_instance(assets, gfx, ());

        // Create `opaque_bind_group`
        let opaque_bind_group =
            VoxelOpaqueBindGroup { depth_texture }.load_instance(assets, gfx, ());

        Self {
            buffer,
            common_bind_group,
            opaque_bind_group,
        }
    }

    pub fn set_camera_matrix(&self, gfx: &GfxContext, camera: glam::Mat4, light: glam::Mat4) {
        self.buffer.write(
            &gfx.queue,
            0,
            &[VoxelCommonUniformData { camera, light }.as_std430()],
        );
    }

    pub fn common_bind_group(&self) -> &BindGroupInstance<VoxelCommonBindGroup<'static>> {
        &self.common_bind_group
    }

    pub fn opaque_bind_group(&self) -> &BindGroupInstance<VoxelOpaqueBindGroup<'static>> {
        &self.opaque_bind_group
    }
}
