use crate::engine::gfx::GfxContext;
use crate::engine::scene::UpdateHandler;
use crate::engine::viewport::ViewportRenderer;
use geode::prelude::*;

pub fn make_game_scene() -> Obj {
	let mut scene = Obj::labeled("game scene root");
	scene.add_alias(move |_cx: &ObjCx| {}, typed_key::<dyn UpdateHandler>());

	scene.add_alias(
		move |cx: &ObjCx, frame: wgpu::SurfaceTexture| {
			let gfx = cx.get::<GfxContext>();

			let frame_view = frame
				.texture
				.create_view(&wgpu::TextureViewDescriptor::default());

			let mut cb = gfx
				.device
				.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

			let pass = cb.begin_render_pass(&wgpu::RenderPassDescriptor {
				label: None,
				color_attachments: &[wgpu::RenderPassColorAttachment {
					view: &frame_view,
					ops: wgpu::Operations {
						load: wgpu::LoadOp::Clear(wgpu::Color {
							r: 1.,
							g: 1.,
							b: 1.,
							a: 1.,
						}),
						store: true,
					},
					resolve_target: None,
				}],
				depth_stencil_attachment: None,
			});

			drop(pass);

			gfx.queue.submit([cb.finish()]);
			frame.present();
		},
		typed_key::<dyn ViewportRenderer>(),
	);
	scene
}
