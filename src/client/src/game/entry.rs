use geode::prelude::*;

use crate::engine::{
	entry::MainLockKey, gfx::GfxContext, scene::SceneUpdateHandler, viewport::ViewportRenderHandler,
};

pub fn make_game_entry(s: Session, _main_lock: Lock) -> Owned<Entity> {
	let update_handler = Obj::new(s, |s: Session, _me: Entity, engine_root: Entity| {
		let main_lock = engine_root.get_in(s, proxy_key::<MainLockKey>());

		log::info!("Updating scene. Our main lock is {main_lock:?}");
	})
	.to_unsized::<dyn SceneUpdateHandler>();

	let render_handler = Obj::new(
		s,
		|frame: Option<wgpu::SurfaceTexture>, s: Session, _me, _viewport, engine_root: Entity| {
			let p_gfx = engine_root.get::<GfxContext>(s);
			let frame = match frame {
				Some(frame) => frame,
				None => return,
			};

			let frame_view = frame.texture.create_view(&Default::default());

			let mut cb = p_gfx.device.create_command_encoder(&Default::default());
			let pass = cb.begin_render_pass(&wgpu::RenderPassDescriptor {
				label: None,
				color_attachments: &[wgpu::RenderPassColorAttachment {
					view: &frame_view,
					resolve_target: None,
					ops: wgpu::Operations {
						load: wgpu::LoadOp::Clear(wgpu::Color {
							r: 90. / 255.,
							g: 184. / 255.,
							b: 224. / 255.,
							a: 1.0,
						}),
						store: true,
					},
				}],
				depth_stencil_attachment: None,
			});

			drop(pass);

			p_gfx.queue.submit([cb.finish()]);
			frame.present();
		},
	)
	.to_unsized::<dyn ViewportRenderHandler>();

	Entity::new_with(s, (update_handler, render_handler))
}
