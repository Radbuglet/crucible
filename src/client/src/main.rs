#![feature(backtrace)]
#![feature(decl_macro)]
#![feature(never_type)]

use crate::render::core::context::GfxContext;
use crate::render::core::viewport::{Viewport, ViewportManager};
use crate::util::winit::{WinitEvent, WinitEventBundle};
use anyhow::Context;
use core::foundation::prelude::*;
use core::util::error::ErrorFormatExt;
use futures::executor::block_on;
use std::sync::Arc;
use winit::dpi::LogicalSize;
use winit::event_loop::EventLoop;
use winit::window::WindowBuilder;

mod render;
mod util;

fn main() {
	if let Err(err) = main_inner() {
		eprintln!("{}", err.format_error(true));
	}
}

fn main_inner() -> anyhow::Result<!> {
	// Start up foundation
	let engine = Arc::new(MultiProvider((
		// Foundation
		MultiProvider::<(
			Component<RwLockManager>,
			Component<Executor>,
			LazyComponent<RwLock<World>>,
		)>::default(),
		// Core services
		MultiProvider::<(
			LazyComponent<GfxContext>,
			LazyComponent<RwLock<ViewportManager>>,
		)>::default(),
	)));

	engine.init_lock(World::default());

	// Set up core rendering services
	let event_loop = EventLoop::new();
	{
		// Create window
		let window = WindowBuilder::new()
			.with_title("Crucible")
			.with_inner_size(LogicalSize::new(1920, 1080))
			.with_visible(false)
			.build(&event_loop)
			.context("Failed to create main window.")?;

		// Create gfx and surface
		let (gfx, surface) = block_on(GfxContext::with_window(&window))?;
		engine.init(gfx);

		// Create VM and viewport
		let mut vm = ViewportManager::new();
		let entity = RwGuard::<&mut World>::lock_now(engine.get()).get().spawn();

		vm.register_pair(engine.get_many(), entity, window, surface);
		engine.init_lock(vm);

		entity
	};

	// Create a second window
	{
		let entity = RwGuard::<&mut World>::lock_now(engine.get()).get().spawn();
		let window = WindowBuilder::new()
			.with_title("Test window")
			.with_inner_size(LogicalSize::new(200, 200))
			.with_visible(false)
			.build(&event_loop)
			.context("Failed to create secondary window.")?;

		RwGuard::<&mut ViewportManager>::lock_now(engine.get())
			.get()
			.register(engine.get_many(), entity, window);
		entity
	};

	// === Start engine ===
	// Make all windows visible
	{
		let guard = RwGuard::<(&mut ViewportManager,)>::lock_now(engine.get_many());
		let (vm,) = guard.get();
		for ent in vm.get_entities() {
			vm.get_viewport(ent).unwrap().window().set_visible(true);
		}
	}

	// Bind event loop
	event_loop.run(move |ev, proxy, flow| {
		let bundle: WinitEventBundle = (&ev, proxy, flow);

		// Handle core events
		if let WinitEvent::MainEventsCleared = &ev {
			let vm = engine.get_lock::<ViewportManager>().lock_ref_now();
			for win_ent in vm.get().get_entities() {
				vm.get()
					.get_viewport(win_ent)
					.unwrap()
					.window()
					.request_redraw();
			}
		}

		// Handle redraws
		{
			// Fetch dependencies
			let guard = RwGuard::<(&mut ViewportManager,)>::lock_now(engine.get_many());
			let gfx: &GfxContext = engine.get();
			let (vm,) = guard.get();

			// Collect redraw requests
			let mut on_redraw = EventPusherPoll::new();
			vm.handle_ev((gfx,), bundle, &mut on_redraw);

			// Handle
			for (_, frame) in on_redraw.drain() {
				let frame_view = frame
					.texture
					.create_view(&wgpu::TextureViewDescriptor::default());

				let mut cb = gfx
					.device
					.create_command_encoder(&wgpu::CommandEncoderDescriptor {
						label: Some("main frame encoder"),
					});

				let mut pass = cb.begin_render_pass(&wgpu::RenderPassDescriptor {
					label: None,
					color_attachments: &[wgpu::RenderPassColorAttachment {
						view: &frame_view,
						resolve_target: None,
						ops: wgpu::Operations {
							load: wgpu::LoadOp::Clear(wgpu::Color {
								r: 0.1,
								g: 0.3,
								b: 0.8,
								a: 1.0,
							}),
							store: true,
						},
					}],
					depth_stencil_attachment: None,
				});
				drop(pass);
				gfx.queue.submit([cb.finish()]);
			}
		}
	})
}
