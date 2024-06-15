use bevy_autoken::{random_component, Obj, RandomEntityExt};
use bevy_ecs::entity::Entity;
use main_loop::{AssetManager, GfxContext};
use shaders::skybox::load_skybox_pipeline;

pub mod shaders;

// === ViewportRenderer === //

pub type ViewportRendererCx = (&'static mut ViewportRenderer,);

#[derive(Debug)]
pub struct ViewportRenderer {
    assets: Obj<AssetManager>,
    gfx: GfxContext,
}

random_component!(ViewportRenderer);

impl ViewportRenderer {
    pub fn new(world: Entity) -> Self {
        let assets = world.get::<AssetManager>();
        let gfx = (*world.get::<GfxContext>()).clone();
        let skybox = load_skybox_pipeline(&assets, &gfx, wgpu::TextureFormat::Bgra8Unorm);

        Self { assets, gfx }
    }

    pub fn render(&mut self, cmd: &mut wgpu::CommandEncoder, frame: &wgpu::TextureView) {
        let pass = cmd.begin_render_pass(&wgpu::RenderPassDescriptor {
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
    }
}
