use anyhow::Context;
use crucible_core::{
	debug::userdata::Userdata,
	ecs::{
		context::Provider,
		core::{Archetype, Storage},
	},
};
use wgpu::SurfaceConfiguration;
use winit::{dpi::LogicalSize, event_loop::EventLoop, window::WindowBuilder};

use crate::game::entry::PlayScene;

use super::{
	gfx::{GfxContext, GfxFeatureNeedsScreen},
	input::InputManager,
	resources::ResourceManager,
	scene::{SceneManager, SceneRenderHandler, SceneUpdateHandler},
	viewport::{FullScreenTexture, Viewport, ViewportManager},
};

struct EngineRoot {
	// Services
	gfx: GfxContext,
	res_mgr: ResourceManager,
	viewport_mgr: ViewportManager,
	scene_mgr: SceneManager,

	// Archetypes
	_viewport_arch: Archetype,
	scene_arch: Archetype,

	// Storages
	viewports: Storage<Viewport>,
	depth_textures: Storage<FullScreenTexture>,
	input_managers: Storage<InputManager>,
	userdata: Storage<Userdata>,
	update_handlers: Storage<SceneUpdateHandler>,
	render_handlers: Storage<SceneRenderHandler>,
}

impl EngineRoot {
	async fn new() -> anyhow::Result<(EventLoop<()>, Self)> {
		// Create main window
		let event_loop = EventLoop::new();
		let main_window = WindowBuilder::new()
			.with_title("Crucible")
			.with_inner_size(LogicalSize::new(1920, 1080))
			.with_visible(false)
			.build(&event_loop)
			.context("failed to create main window")?;

		// Create graphics subsystem
		let (gfx, _compat, main_surface) =
			GfxContext::init(&main_window, &mut GfxFeatureNeedsScreen)
				.await
				.context("failed to create graphics context")?;

		// Create main viewport
		let mut viewport_mgr = ViewportManager::default();
		let mut viewport_arch = Archetype::default();
		let mut viewports = Storage::new();
		let mut depth_textures = Storage::new();
		let mut input_managers = Storage::new();

		let main_viewport = viewport_arch.spawn("main viewport");
		viewports.add(
			main_viewport,
			Viewport::new(
				(&gfx,),
				main_window,
				Some(main_surface),
				SurfaceConfiguration {
					alpha_mode: wgpu::CompositeAlphaMode::Auto,
					format: wgpu::TextureFormat::Bgra8UnormSrgb,
					present_mode: wgpu::PresentMode::Fifo,
					usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
					width: 0,
					height: 0,
				},
			),
		);
		depth_textures.add(
			main_viewport,
			FullScreenTexture::new(
				"depth texture",
				wgpu::TextureFormat::Depth32Float,
				wgpu::TextureUsages::RENDER_ATTACHMENT,
			),
		);
		input_managers.add(main_viewport, InputManager::default());

		viewport_mgr.register((&viewports,), main_viewport);

		// Create scene manager
		let scene_mgr = SceneManager::default();
		let scene_arch = Archetype::default();
		let userdata = Storage::new();
		let update_handlers = Storage::new();
		let render_handlers = Storage::new();

		// Create resource manager
		let res_mgr = ResourceManager::default();

		Ok((
			event_loop,
			Self {
				// Services
				gfx,
				res_mgr,
				viewport_mgr,
				scene_mgr,

				// Archetypes
				_viewport_arch: viewport_arch,
				scene_arch,

				// Storages
				viewports,
				depth_textures,
				input_managers,
				userdata,
				update_handlers,
				render_handlers,
			},
		))
	}
}

pub async fn main_inner() -> anyhow::Result<()> {
	let (event_loop, mut root) = EngineRoot::new().await?;

	// Make all viewports visible
	{
		let viewports = &mut root.viewports;
		for &viewport in root.viewport_mgr.window_map().values() {
			viewports[viewport].window().set_visible(true);
		}
	}

	// Setup initial scene
	{
		let scene_mgr = &mut root.scene_mgr;
		let main_viewport = root
			.viewport_mgr
			.window_map()
			.values()
			.copied()
			.next()
			.unwrap();

		let scene = PlayScene::spawn(
			(
				&mut root.scene_arch,
				&mut root.userdata,
				&mut root.update_handlers,
				&mut root.render_handlers,
				&root.gfx,
				&mut root.res_mgr,
			),
			main_viewport,
		);

		scene_mgr.set_initial(scene);
	}

	// Run event loop
	event_loop.run(move |event, _proxy, flow| {
		use winit::event::{Event::*, WindowEvent::*};

		flow.set_poll();

		match event {
			// First window events and device events are dispatched.
			WindowEvent { window_id, event } => {
				let Some(viewport) = root.viewport_mgr.get_viewport(window_id) else {
					return;
				};

				// Process window event
				root.input_managers[viewport].handle_window_event(&event);

				if let CloseRequested = event {
					flow.set_exit();
				}
			}
			DeviceEvent { device_id, event } => {
				for (_, &viewport) in root.viewport_mgr.window_map() {
					root.input_managers[viewport].handle_device_event(device_id, &event);
				}
			}
			MainEventsCleared => {
				// Process update
				let curr_scene = root.scene_mgr.current();
				let mut cx = (
					&root.gfx,
					&mut root.viewport_mgr,
					&mut root.depth_textures,
					&mut root.input_managers,
					&mut root.viewports,
					&mut root.userdata,
					&mut root.res_mgr,
				);

				root.update_handlers[curr_scene](curr_scene, &mut cx.as_dyn());

				// Request redraws
				for (&_window, &viewport) in root.viewport_mgr.window_map() {
					root.viewports[viewport].window().request_redraw();
				}

				// Clear input queue
				for (_, &viewport) in root.viewport_mgr.window_map() {
					root.input_managers[viewport].end_tick();
				}
			}

			// Then, redraws are processed.
			RedrawRequested(window_id) => {
				let Some(viewport) = root.viewport_mgr.get_viewport(window_id) else {
					return;
				};

				// Acquire frame
				let viewport_data = &mut root.viewports[viewport];
				let Ok(surface) = viewport_data.present((&root.gfx,)) else {
					log::error!("Failed to render to {viewport:?}");
					return;
				};

				let Some(mut frame) = surface else {
					return;
				};

				// Process render
				let curr_scene = root.scene_mgr.current();
				let mut cx = (
					&mut root.userdata,
					&root.gfx,
					&mut root.res_mgr,
					&mut root.viewports,
					&mut root.depth_textures,
				);
				root.render_handlers[curr_scene](curr_scene, &mut cx.as_dyn(), &mut frame);

				frame.present();
			}
			RedrawEventsCleared => {}

			// This runs at program termination.
			LoopDestroyed => {
				log::info!("Goodbye!");
			}
			_ => {}
		}
	});
}
