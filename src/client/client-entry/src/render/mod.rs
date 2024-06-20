use std::time::Duration;

use bevy_autoken::{random_component, Obj, RandomEntityExt};
use bevy_ecs::entity::Entity;
use crucible_assets::AssetManager;
use helpers::{AtlasTexture, AtlasTextureGfx, CameraManager, FullScreenTexture};
use main_loop::{GfxContext, Viewport};
use shaders::{
    skybox::{load_skybox_pipeline, SkyboxUniforms},
    voxel::{load_opaque_block_pipeline, VoxelUniforms},
};
use typed_glam::glam::UVec2;
use voxel::WorldVoxelMesh;
use wgpu::util::DeviceExt;

pub mod helpers;
pub mod shaders;
pub mod voxel;

// === ViewportRenderer === //

const MESH_TIME_LIMIT: Option<Duration> = Some(Duration::from_millis(10));

pub type ViewportRendererCx = (&'static mut ViewportRenderer,);

#[derive(Debug)]
pub struct ViewportRenderer {
    // Services
    assets: Obj<AssetManager>,
    gfx: GfxContext,
    camera: Obj<CameraManager>,

    // Atlas
    atlas: AtlasTexture,
    atlas_gfx: AtlasTextureGfx,

    // Depth
    depth: FullScreenTexture,

    // Rendering subsystems
    skybox: SkyboxUniforms,
    voxel: Obj<WorldVoxelMesh>,
    voxel_uniforms: VoxelUniforms,
}

random_component!(ViewportRenderer);

impl ViewportRenderer {
    pub fn new(engine_root: Entity) -> Self {
        // Fetch services
        let assets = engine_root.get::<AssetManager>();
        let gfx = (*engine_root.get::<GfxContext>()).clone();
        let camera = engine_root.get::<CameraManager>();

        // Generate atlas textures
        let atlas = AtlasTexture::new(UVec2::splat(100), UVec2::splat(32), 4);
        let atlas_gfx = AtlasTextureGfx::new(&gfx, &atlas, Some("voxel texture atlas"));

        // Generate depth texture
        let depth = FullScreenTexture::new(
            Some("depth texture"),
            wgpu::TextureFormat::Depth32Float,
            wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::RENDER_ATTACHMENT,
        );

        // load skybox subsystem
        let skybox = image::load_from_memory(include_bytes!("embedded_res/default_skybox.png"))
            .unwrap()
            .into_rgba8();

        let skybox = gfx.device.create_texture_with_data(
            &gfx.queue,
            &wgpu::TextureDescriptor {
                label: Some("Skybox panorama"),
                size: wgpu::Extent3d {
                    width: skybox.width(),
                    height: skybox.height(),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &skybox,
        );
        let skybox = skybox.create_view(&wgpu::TextureViewDescriptor::default());
        let skybox = SkyboxUniforms::new(&assets, &gfx, &skybox);

        // Load voxel subsystem
        let voxel = engine_root.get::<WorldVoxelMesh>();
        let voxel_uniforms = VoxelUniforms::new(&assets, &gfx, &atlas_gfx.view);

        Self {
            // Services
            assets,
            gfx,
            camera,

            // Atlas
            atlas,
            atlas_gfx,

            // Depth
            depth,

            // Rendering subsystems
            skybox,
            voxel,
            voxel_uniforms,
        }
    }

    pub fn render(
        &mut self,
        cmd: &mut wgpu::CommandEncoder,
        viewport: &Viewport,
        frame: &wgpu::TextureView,
    ) {
        self.camera.recompute();
        let proj_xform = self
            .camera
            .get_camera_xform(viewport.curr_surface_aspect().unwrap_or(1.));

        let skybox = load_skybox_pipeline(&self.assets, &self.gfx, viewport.curr_config().format);
        let voxels = load_opaque_block_pipeline(
            &self.assets,
            &self.gfx,
            viewport.curr_config().format,
            self.depth.format(),
        );
        let voxels_pass = self.voxel.prepare_pass();

        // Draw skybox
        let mut pass = cmd.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("skybox pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: frame,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        skybox.bind_pipeline(&mut pass);

        self.skybox.set_camera_matrix(&self.gfx, proj_xform);
        self.skybox.write_pass_state(&mut pass);
        pass.draw(0..6, 0..1);

        drop(pass);

        // Draw voxels
        let mut pass = cmd.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("voxel pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: frame,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: self.depth.acquire_view(&self.gfx, viewport),
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        self.voxel_uniforms.set_camera_matrix(&self.gfx, proj_xform);
        self.voxel.update(&self.gfx, &self.atlas, MESH_TIME_LIMIT);
        voxels_pass.render(&voxels, &self.voxel_uniforms, &mut pass);

        drop(pass);
    }
}
