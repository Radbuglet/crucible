use anyhow::Context;
use crucible_core::{
	debug::userdata::Userdata,
	ecs::{entity::Archetype, provider::Provider, storage::Storage},
};
use wgpu::SurfaceConfiguration;
use winit::{
	dpi::LogicalSize,
	event::{DeviceEvent, DeviceId, WindowEvent},
	event_loop::EventLoopBuilder,
	window::{WindowBuilder, WindowId},
};

use crate::{
	engine::scene::{SceneRenderEvent, SceneUpdateEvent},
	game::entry::PlayScene,
};

use super::{
	io::{
		gfx::{GfxContext, GfxFeatureNeedsScreen},
		input::InputManager,
		main_loop::{MainLoop, MainLoopHandler, WinitEventLoop, WinitEventProxy},
		viewport::{FullScreenTexture, Viewport, ViewportManager},
	},
	resources::ResourceManager,
	scene::{SceneManager, SceneRenderHandler, SceneUpdateHandler},
};

// === EngineRoot === //

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
	async fn new() -> anyhow::Result<(WinitEventLoop, Self)> {
		// Create main window
		let event_loop = EventLoopBuilder::with_user_event().build();
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

impl MainLoopHandler for EngineRoot {
	fn on_update(&mut self, _cx: (&mut MainLoop, &WinitEventProxy)) {
		let mut cx = (
			&self.gfx,
			&mut self.viewport_mgr,
			&mut self.depth_textures,
			&mut self.input_managers,
			&mut self.viewports,
			&mut self.userdata,
			&mut self.res_mgr,
		);

		let scene = self.scene_mgr.current();
		self.update_handlers[scene](&mut cx.as_dyn(), scene, SceneUpdateEvent {});

		// Request redraws
		for (&_window, &viewport) in self.viewport_mgr.window_map() {
			self.viewports[viewport].window().request_redraw();
		}

		// Clear input queue
		for (_, &viewport) in self.viewport_mgr.window_map() {
			self.input_managers[viewport].end_tick();
		}
	}

	fn on_render(&mut self, _cx: (&mut MainLoop, &WinitEventProxy), window_id: WindowId) {
		let Some(viewport) = self.viewport_mgr.get_viewport(window_id) else {
			return;
		};

		// Acquire frame
		let viewport_data = &mut self.viewports[viewport];
		let Ok(surface) = viewport_data.present((&self.gfx,)) else {
			log::error!("Failed to render to {viewport:?}");
			return;
		};

		let Some(mut frame) = surface else {
			return;
		};

		// Process render
		let curr_scene = self.scene_mgr.current();
		let mut cx = (
			&mut self.userdata,
			&self.gfx,
			&mut self.res_mgr,
			&mut self.viewports,
			&mut self.depth_textures,
		);
		self.render_handlers[curr_scene](
			&mut cx.as_dyn(),
			curr_scene,
			SceneRenderEvent { frame: &mut frame },
		);

		frame.present();
	}

	fn on_window_input(
		&mut self,
		(main_loop, _proxy): (&mut MainLoop, &WinitEventProxy),
		window_id: WindowId,
		event: WindowEvent,
	) {
		let Some(viewport) = self.viewport_mgr.get_viewport(window_id) else {
			log::warn!("Unknown viewport with window ID {window_id:?}");
			return;
		};

		self.input_managers[viewport].handle_window_event(&event);

		if let WindowEvent::CloseRequested = &event {
			main_loop.exit();
		}
	}

	fn on_device_input(
		&mut self,
		_cx: (&mut MainLoop, &WinitEventProxy),
		device_id: DeviceId,
		event: DeviceEvent,
	) {
		for &viewport in self.viewport_mgr.window_map().values() {
			self.input_managers[viewport].handle_device_event(device_id, &event);
		}
	}
}

// === Main === //

pub async fn main_inner() -> anyhow::Result<()> {
	// Construct engine root
	let (event_loop, mut root) = EngineRoot::new().await?;

	// Setup initial scene
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

	// Make all viewports visible
	let viewports = &mut root.viewports;
	for &viewport in root.viewport_mgr.window_map().values() {
		viewports[viewport].window().set_visible(true);
	}

	// Run event loop
	MainLoop::start(event_loop, root);
}
