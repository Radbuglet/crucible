use crate::engine::gfx::GfxContext;
use crate::engine::viewport::{Viewport, ViewportManager};
use crate::util::winit::{WinitEventBundle, WinitEventHandler};
use geode::prelude::*;
use winit::event::{Event, WindowEvent};
use winit::event_loop::ControlFlow;

pub fn make_game_scene() -> Obj {
	let mut scene = Obj::labeled("game scene root");
	scene.add_alias(
		|cx: &mut ObjCx, bundle: &mut WinitEventBundle| {
			let gfx = cx.get::<GfxContext>();
			match &bundle.event {
				Event::RedrawRequested(win_id) => {
					// Acquire viewport
					let vm = cx.borrow::<ViewportManager>();
					let viewport_obj = match vm.get_viewport(*win_id) {
						Some(obj) => obj,
						None => return,
					};
					let mut viewport = viewport_obj.borrow_mut::<Viewport>();

					// Acquire frame
					let frame = match viewport.render(gfx).expect("surface lost") {
						Some(frame) => frame,
						None => return,
					};
					let frame_view = frame.texture.create_view(&Default::default());

					// Create command buffer
					let mut cb = gfx.device.create_command_encoder(&Default::default());
					let pass = cb.begin_render_pass(&wgpu::RenderPassDescriptor {
						label: Some("main pass"),
						color_attachments: &[wgpu::RenderPassColorAttachment {
							view: &frame_view,
							resolve_target: None,
							ops: wgpu::Operations {
								load: wgpu::LoadOp::Clear(wgpu::Color {
									r: 0.05,
									g: 0.1,
									b: 0.7,
									a: 1.,
								}),
								store: true,
							},
						}],
						depth_stencil_attachment: None,
					});
					drop(pass);

					gfx.queue.submit([cb.finish()]);
					frame.present();
				}
				Event::WindowEvent {
					window_id: win_id,
					event,
				} => {
					let mut vm = cx.borrow_mut::<ViewportManager>();
					let viewport_obj = match vm.get_viewport(*win_id) {
						Some(obj) => obj,
						None => return,
					};

					match event {
						WindowEvent::CloseRequested => {
							vm.unregister(*win_id);
							if vm.viewports().next().is_none() {
								*bundle.flow = ControlFlow::Exit;
							}
						}
						_ => {}
					}
				}
				_ => {}
			}
		},
		typed_key::<dyn WinitEventHandler>(),
	);

	scene
}
