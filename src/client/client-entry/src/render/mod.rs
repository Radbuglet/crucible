use bevy_autoken::random_component;
use main_loop::GfxContext;

pub type ViewportRendererCx = (&'static mut ViewportRenderer,);

#[derive(Debug)]
pub struct ViewportRenderer {
    gfx: GfxContext,
}

random_component!(ViewportRenderer);

impl ViewportRenderer {
    pub fn new(gfx: GfxContext) -> Self {
        Self { gfx }
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
