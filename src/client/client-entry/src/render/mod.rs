use bevy_autoken::{random_component, Obj, RandomEntityExt};
use bevy_ecs::entity::Entity;
use crucible_assets::AssetManager;
use crucible_math::{Angle3D, Angle3DExt};
use helpers::{CameraManager, CameraSettings};
use main_loop::{GfxContext, Viewport};
use shaders::skybox::{load_skybox_pipeline, SkyboxUniforms};
use typed_glam::glam::Vec3;
use wgpu::util::DeviceExt;

pub mod helpers;
pub mod shaders;

// === ViewportRenderer === //

pub type ViewportRendererCx = (&'static mut ViewportRenderer,);

#[derive(Debug)]
pub struct ViewportRenderer {
    assets: Obj<AssetManager>,
    gfx: GfxContext,
    camera: CameraManager,
    skybox: SkyboxUniforms,
    time: f32,
}

random_component!(ViewportRenderer);

impl ViewportRenderer {
    pub fn new(world: Entity) -> Self {
        let assets = world.get::<AssetManager>();
        let gfx = (*world.get::<GfxContext>()).clone();
        let camera = CameraManager::default();

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

        Self {
            assets,
            gfx,
            camera,
            skybox,
            time: 0.,
        }
    }

    pub fn render(
        &mut self,
        cmd: &mut wgpu::CommandEncoder,
        viewport: &Viewport,
        frame: &wgpu::TextureView,
    ) {
        self.camera.unset();
        self.camera.set_pos_rot(
            Vec3::ZERO,
            Angle3D::new_deg(self.time, 0.),
            CameraSettings::Perspective {
                fov: 90.,
                near: 0.1,
                far: 90.,
            },
        );
        self.time += 1.;

        let skybox = load_skybox_pipeline(&self.assets, &self.gfx, viewport.curr_config().format);

        let mut pass = cmd.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("voxel render"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: frame,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.02,
                        g: 0.01,
                        b: 0.05,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        skybox.bind_pipeline(&mut pass);

        self.skybox.set_camera_matrix(
            &self.gfx,
            self.camera
                .get_camera_xform(viewport.curr_surface_aspect().unwrap_or(1.)),
        );
        self.skybox.write_pass_state(&mut pass);
        pass.draw(0..6, 0..1);
    }
}
