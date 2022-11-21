use crucible_core::ecs::{
	context::{unpack, DynProvider},
	core::{Archetype, Entity, Storage},
	userdata::Userdata,
};

use crate::engine::{
	gfx::GfxContext,
	scene::{SceneRenderHandler, SceneUpdateHandler},
};

#[derive(Debug, Default)]
pub struct PlayScene {
	time: f64,
}

impl PlayScene {
	pub fn spawn(
		(scene_arch, userdatas, update_handlers, render_handlers): (
			&mut Archetype,
			&mut Storage<Userdata>,
			&mut Storage<SceneUpdateHandler>,
			&mut Storage<SceneRenderHandler>,
		),
	) -> Entity {
		let scene = scene_arch.spawn();

		userdatas.add(scene, Box::new(Self::default()));
		update_handlers.add(scene, Self::on_update);
		render_handlers.add(scene, Self::on_render);

		scene
	}

	pub fn on_update(me: Entity, cx: &mut DynProvider) {
		unpack!(cx => {
			userdatas = &mut Storage<Userdata>,
		});

		let me = userdatas.get_downcast_mut::<Self>(me);
		me.time += 0.1;
	}

	pub fn on_render(me: Entity, cx: &mut DynProvider, frame: &mut wgpu::SurfaceTexture) {
		unpack!(cx => {
			userdata = &Storage<Userdata>,
			gfx = &GfxContext,
		});

		let me_data = userdata.get_downcast::<Self>(me);

		let view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
			label: Some("frame view"),
			format: None,
			dimension: None,
			aspect: wgpu::TextureAspect::All,
			base_mip_level: 0,
			mip_level_count: None,
			base_array_layer: 0,
			array_layer_count: None,
		});

		let mut encoder = gfx
			.device
			.create_command_encoder(&wgpu::CommandEncoderDescriptor {
				label: Some("main viewport renderer"),
			});

		let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
			label: Some("main render pass"),
			color_attachments: &[Some(wgpu::RenderPassColorAttachment {
				view: &view,
				ops: wgpu::Operations {
					load: wgpu::LoadOp::Clear(wgpu::Color {
						r: 0.5 + 0.5 * me_data.time.cos(),
						g: 0.1,
						b: 0.1,
						a: 1.0,
					}),
					store: true,
				},
				resolve_target: None,
			})],
			depth_stencil_attachment: None,
		});

		drop(pass);

		gfx.queue.submit([encoder.finish()]);
	}
}
