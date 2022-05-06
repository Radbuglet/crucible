use crate::engine::services::gfx::{
	CompatQueryInfo, GfxContext, GfxFeatureDetector, GfxFeatureNeedsScreen,
	GfxFeaturePowerPreference,
};
use crate::engine::services::viewport::{Viewport, ViewportManager};
use crate::util::features::FeatureList;
use anyhow::Context;
use futures::executor::block_on;
use geode::prelude::*;
use winit::dpi::LogicalSize;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;

pub fn main_inner() -> anyhow::Result<()> {
	// Initialize logger
	env_logger::init();
	log::info!("Hello!");

	// Initialize engine
	let event_loop = EventLoop::new();
	let root = block_on(make_engine_root(&event_loop)).context("failed to initialize engine")?;

	// Run engine
	log::info!("Main loop starting.");
	root.inject(|vm: ARef<ViewportManager>| {
		for (_, viewport_obj) in vm.viewports() {
			viewport_obj.borrow::<Viewport>().window().set_visible(true);
		}
	});
	event_loop.run(move |event, _proxy, flow| {
		let gfx = root.get::<GfxContext>();
		let mut vm = root.borrow_mut::<ViewportManager>();

		match &event {
			Event::RedrawRequested(window_id) => {
				let viewport_obj = match vm.get_viewport(*window_id) {
					Some(viewport) => viewport,
					None => return,
				};
				let mut viewport = viewport_obj.borrow_mut::<Viewport>();
				if let Some(frame) = viewport.render(gfx).unwrap() {
					let view = frame.texture.create_view(&Default::default());

					let mut cb =
						gfx.device
							.create_command_encoder(&wgpu::CommandEncoderDescriptor {
								label: Some("frame command encoder"),
							});

					let mut pass = cb.begin_render_pass(&wgpu::RenderPassDescriptor {
						label: Some("main render pass"),
						color_attachments: &[wgpu::RenderPassColorAttachment {
							view: &view,
							resolve_target: None,
							ops: wgpu::Operations {
								load: wgpu::LoadOp::Clear(wgpu::Color {
									r: 0.2,
									g: 0.2,
									b: 0.2,
									a: 1.0,
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
			}
			Event::WindowEvent { window_id, event } => {
				let viewport_obj = match vm.get_viewport(*window_id) {
					Some(viewport) => viewport,
					None => return,
				};
				let mut viewport = viewport_obj.borrow_mut::<Viewport>();

				match event {
					WindowEvent::CloseRequested => {
						drop(viewport);
						vm.unregister(*window_id);

						if vm.viewports().next().is_none() {
							*flow = ControlFlow::Exit;
						}
					}
					_ => {}
				}
			}
			Event::MainEventsCleared => {
				for (_, viewport_obj) in vm.viewports() {
					viewport_obj.borrow::<Viewport>().window().request_redraw();
				}
			}
			Event::LoopDestroyed => {
				log::info!("Goodbye!");
			}
			_ => {}
		}
	});
}

async fn make_engine_root(event_loop: &EventLoop<()>) -> anyhow::Result<Obj> {
	let mut root = Obj::new();

	// Create core services
	root.add_rw(World::new());

	// Create graphics subsystem
	let (main_window, main_swapchain) = {
		let main_window = WindowBuilder::new()
			.with_title("Crucible")
			.with_inner_size(LogicalSize::new(1920u32, 1080u32))
			.with_visible(false)
			.build(event_loop)
			.context("failed to create main window")?;

		let (gfx, _gfx_features, main_swapchain) =
			GfxContext::init(&main_window, &mut MyFeatureListValidator)
				.await
				.context("failed to create graphics context")?;

		root.add(gfx);
		(main_window, main_swapchain)
	};
	{
		let gfx = root.get::<GfxContext>();
		let mut vm = ViewportManager::default();
		vm.register(gfx, Obj::new(), main_window, main_swapchain);
		root.add_rw(vm);
	}

	Ok(root)
}

struct MyFeatureListValidator;

impl GfxFeatureDetector for MyFeatureListValidator {
	type Table = ();

	fn query_compat(&mut self, info: &mut CompatQueryInfo) -> (FeatureList, Option<Self::Table>) {
		let mut features = FeatureList::default();
		features.import_from(GfxFeatureNeedsScreen.query_compat(info).0);
		features.import_from(
			GfxFeaturePowerPreference(wgpu::PowerPreference::HighPerformance)
				.query_compat(info)
				.0,
		);
		features.wrap_user_table(())
	}
}
