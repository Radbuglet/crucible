#![feature(backtrace)]
#![feature(decl_macro)]
#![feature(never_type)]

use crate::render::core::context::GfxContext;
use crate::render::core::run_loop::{start_run_loop, RunLoopEvent, RunLoopHandler};
use crate::render::core::viewport::ViewportManager;
use anyhow::Context;
use core::foundation::prelude::*;
use core::util::error::ErrorFormatExt;
use futures::executor::block_on;
use std::sync::Arc;
use wgpu::SurfaceTexture;
use winit::dpi::LogicalSize;
use winit::event::{DeviceEvent, DeviceId, WindowEvent};
use winit::event_loop::{EventLoop, EventLoopWindowTarget};
use winit::window::WindowBuilder;

mod render;
mod util;

fn main() {
	if let Err(err) = block_on(main_inner()) {
		eprintln!("{}", err.format_error(true));
	}
}

type Engine = Arc<
	MultiProvider<(
		// Foundational services
		Component<Executor>,
		Component<RwLockManager>,
		LazyComponent<RwLock<World>>,
		// Graphics services
		LazyComponent<GfxContext>,
		LazyComponent<RwLock<ViewportManager>>,
	)>,
>;

async fn main_inner() -> anyhow::Result<!> {
	// Initialize foundational services
	env_logger::init();

	let engine = Engine::default();
	let mut world = World::new();

	// Startup graphics singleton and create the main window
	log::info!("Initializing graphics subsystem...");
	log::info!("Creating EventLoop");
	let event_loop = EventLoop::new();
	let (gfx, vm) = {
		log::info!("Creating main window");
		let window = WindowBuilder::new()
			.with_title("Crucible")
			.with_visible(false)
			.with_inner_size(LogicalSize::new(1920, 1080))
			.build(&event_loop)
			.context("Failed to create main window.")?;

		log::info!("Initializing wgpu context");
		let (gfx, surface) = GfxContext::with_window(&window)
			.await
			.context("Failed to initialize wgpu!")?;

		let mut vm = ViewportManager::new();
		let entity = world.spawn();
		vm.register_pair(&gfx, entity, window, surface);

		(gfx, vm)
	};
	log::info!("Done initializing graphics subsystem!");

	// Setup engine
	for e_win in vm.get_entities() {
		vm.get_viewport(e_win).unwrap().window().set_visible(true);
	}

	engine.init_lock(world);
	engine.init(gfx);
	engine.init_lock(vm);

	// Start
	log::info!("Starting run loop!");
	start_run_loop(event_loop, engine, Handler);
}

struct Handler;

impl RunLoopHandler for Handler {
	type Engine = Engine;

	fn tick(
		&mut self,
		_ev_pusher: &mut EventPusherPoll<RunLoopEvent>,
		_engine: &Self::Engine,
		_event_loop: &EventLoopWindowTarget<()>,
		_vm_guard: RwGuardMut<ViewportManager>,
	) {
		log::trace!("Tick!");
	}

	fn draw(
		&mut self,
		_ev_pusher: &mut EventPusherPoll<RunLoopEvent>,
		engine: &Self::Engine,
		_event_loop: &EventLoopWindowTarget<()>,
		_vm_guard: RwGuardMut<ViewportManager>,
		_window: Entity,
		frame: SurfaceTexture,
	) {
		let gfx: &GfxContext = engine.get();

		let frame_view = frame
			.texture
			.create_view(&wgpu::TextureViewDescriptor::default());

		let mut cb = gfx
			.device
			.create_command_encoder(&wgpu::CommandEncoderDescriptor {
				label: Some("primary command encoder"),
			});

		let pass = cb.begin_render_pass(&wgpu::RenderPassDescriptor {
			label: None,
			color_attachments: &[wgpu::RenderPassColorAttachment {
				view: &frame_view,
				ops: wgpu::Operations {
					load: wgpu::LoadOp::Clear(wgpu::Color {
						r: 0.2,
						g: 0.4,
						b: 0.8,
						a: 1.0,
					}),
					store: true,
				},
				resolve_target: None,
			}],
			depth_stencil_attachment: None,
		});

		drop(pass);

		gfx.queue.submit([cb.finish()]);
	}

	fn window_input(
		&mut self,
		ev_pusher: &mut EventPusherPoll<RunLoopEvent>,
		_engine: &Self::Engine,
		_event_loop: &EventLoopWindowTarget<()>,
		vm_guard: RwGuardMut<ViewportManager>,
		window: Entity,
		event: &WindowEvent,
	) {
		let vm = vm_guard.get();
		if let WindowEvent::CloseRequested = event {
			vm.unregister(vm.get_viewport(window).unwrap().id());

			if vm.get_entities().len() == 0 {
				ev_pusher.push(RunLoopEvent::Shutdown);
			}
		}
	}

	fn device_input(
		&mut self,
		_ev_pusher: &mut EventPusherPoll<RunLoopEvent>,
		_engine: &Self::Engine,
		_event_loop: &EventLoopWindowTarget<()>,
		_vm_guard: RwGuardMut<ViewportManager>,
		_device_id: DeviceId,
		_event: &DeviceEvent,
	) {
	}

	fn goodbye(&mut self, _engine: &Self::Engine, _vm_guard: RwGuardMut<ViewportManager>) {}
}
