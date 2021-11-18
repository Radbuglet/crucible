#![allow(dead_code)]
#![feature(backtrace)]
#![feature(decl_macro)]
#![feature(never_type)]

use crate::engine::context::GfxContext;
use crate::engine::run_loop::{start_run_loop, RunLoopCommand, RunLoopHandler};
use crate::engine::util::camera::{GfxCameraManager, PerspectiveCamera};
use crate::engine::util::uniform::UniformManager;
use crate::engine::viewport::ViewportManager;
use crate::voxel::render::VoxelRenderer;
use anyhow::Context;
use cgmath::Vector3;
use crucible_core::foundation::prelude::*;
use crucible_core::util::error::{AnyResult, ErrorFormatExt};
use futures::executor::block_on;
use std::sync::Arc;
use winit::dpi::LogicalSize;
use winit::event::{DeviceEvent, DeviceId, WindowEvent};
use winit::event_loop::{EventLoop, EventLoopWindowTarget};
use winit::window::WindowBuilder;

pub mod engine;
pub mod util;
pub mod voxel;

fn main() {
	if let Err(err) = main_inner() {
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
		LazyComponent<GfxCameraManager>,
		LazyComponent<RwLock<UniformManager>>,
		LazyComponent<RwLock<ViewportManager>>,
		LazyComponent<RwLock<VoxelRenderer>>,
	)>,
>;

fn main_inner() -> AnyResult<!> {
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
			.with_inner_size(LogicalSize::new(1000, 1000))
			.build(&event_loop)
			.context("Failed to create main window.")?;

		log::info!("Initializing wgpu context");
		let (gfx, surface) = block_on(GfxContext::with_window(
			&window,
			wgpu::Features::POLYGON_MODE_LINE,
		))
		.context("Failed to initialize wgpu!")?;

		let mut vm = ViewportManager::new();
		let entity = world.spawn();
		vm.register_pair(&world, &gfx, entity, window, surface);

		(gfx, vm)
	};
	log::info!("Done initializing graphics subsystem!");

	// Setup voxel services
	let uniform = UniformManager::new(
		&gfx,
		Some("uniform manager"),
		wgpu::BufferUsages::UNIFORM,
		1024,
	);

	let camera = GfxCameraManager::new(&gfx);
	let voxel = VoxelRenderer::new(&gfx, &camera);

	// Setup engine
	for e_win in vm.get_entities() {
		vm.get_viewport(e_win).unwrap().window().set_visible(true);
	}

	engine.init_lock(world);
	engine.init_lock(uniform);
	engine.init(gfx);
	engine.init_lock(vm);
	engine.init_lock(voxel);
	engine.init(camera);

	// Start
	log::info!("Starting run loop!");
	start_run_loop(event_loop, engine, Handler);
}

struct Handler;

impl RunLoopHandler for Handler {
	type Engine = Engine;

	fn tick(
		&mut self,
		_ev_pusher: &mut EventPusherPoll<RunLoopCommand>,
		_engine: &Self::Engine,
		_event_loop: &EventLoopWindowTarget<()>,
		_vm_guard: RwGuardMut<ViewportManager>,
	) {
		log::trace!("Tick!");
	}

	fn draw(
		&mut self,
		_ev_pusher: &mut EventPusherPoll<RunLoopCommand>,
		engine: &Self::Engine,
		_event_loop: &EventLoopWindowTarget<()>,
		vm_guard: RwGuardMut<ViewportManager>,
		window: Entity,
		frame: &wgpu::SurfaceTexture,
	) {
		// Lock services
		get_many!(&**engine, gfx: &GfxContext, gfx_camera: &GfxCameraManager);
		lock_many_now!(
			engine.get_many() => _guard,
			uniform: &mut UniformManager,
			voxel: &mut VoxelRenderer,
		);

		// Construct uniforms
		match block_on(uniform.begin_frame()) {
			Ok(_) => {}
			Err(err) => {
				log::warn!("Failed to begin frame {}", err);
				return;
			}
		}

		let viewport = vm_guard.get().get_viewport(window).unwrap();
		let camera = PerspectiveCamera {
			position: Vector3::new(0., 1., 10.),
			..Default::default()
		};

		let camera_group =
			gfx_camera.upload_view(gfx, uniform, camera.get_view_matrix(viewport.aspect()));

		uniform.end_frame();

		// Create view
		let frame_view = frame.texture.create_view(&Default::default());

		// Construct command buffer
		let mut cb = gfx
			.device
			.create_command_encoder(&wgpu::CommandEncoderDescriptor {
				label: Some("primary command encoder"),
			});

		let mut pass = cb.begin_render_pass(&wgpu::RenderPassDescriptor {
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

		voxel.render(&camera_group, &mut pass);
		drop(pass);
		gfx.queue.submit([cb.finish()]);
	}

	fn window_input(
		&mut self,
		ev_pusher: &mut EventPusherPoll<RunLoopCommand>,
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
				ev_pusher.push(RunLoopCommand::Shutdown);
			}
		}
	}

	fn device_input(
		&mut self,
		_ev_pusher: &mut EventPusherPoll<RunLoopCommand>,
		_engine: &Self::Engine,
		_event_loop: &EventLoopWindowTarget<()>,
		_vm_guard: RwGuardMut<ViewportManager>,
		_device_id: DeviceId,
		_event: &DeviceEvent,
	) {
	}

	fn goodbye(&mut self, _engine: &Self::Engine, _vm_guard: RwGuardMut<ViewportManager>) {}
}
