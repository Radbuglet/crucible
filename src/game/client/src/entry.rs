use anyhow::Context;
use bort::{storage, BehaviorRegistry, OwnedEntity};
use crucible_foundation_client::engine::{
	gfx::texture::FullScreenTexture,
	io::{
		gfx::{
			feat_requires_power_pref, feat_requires_screen, CompatQueryInfo, GfxContext, Judgement,
		},
		input::InputManager,
		main_loop::{MainLoop, MainLoopHandler, WinitEventProxy, WinitUserdata},
		viewport::{Viewport, ViewportManager},
	},
	scene::{SceneManager, SceneRenderHandler, SceneUpdateHandler},
};
use winit::{
	dpi::LogicalSize,
	event::WindowEvent,
	event_loop::{EventLoop, EventLoopBuilder},
	window::{WindowBuilder, WindowId},
};

use crate::game::prefabs::scene_root::make_game_scene_root;

pub fn main_inner() -> anyhow::Result<()> {
	// Create the behavior registry
	let bhv = BehaviorRegistry::new().with_many(crate::game::prefabs::register);

	// Create the event loop
	let event_loop: EventLoop<WinitUserdata> = EventLoopBuilder::with_user_event().build();

	// Create the main window
	let main_window = WindowBuilder::new()
		.with_title("Crucible")
		.with_visible(false)
		.with_inner_size(LogicalSize::new(1920, 1080))
		.build(&event_loop)
		.context("failed to create main window")?;

	// Create the graphics context
	let (gfx, main_surface, ()) = futures::executor::block_on(GfxContext::new(
		&main_window,
		|info: &mut CompatQueryInfo| {
			Judgement::new_ok("Adapter is suitable")
				.sub(feat_requires_screen(info).0)
				.sub(
					feat_requires_power_pref(wgpu::PowerPreference::HighPerformance)(info)
						.0
						.make_soft_error(1.0),
				)
				.with_table(())
		},
	))
	.context("failed to create graphics device")?;

	// Create viewport manager
	let mut viewport_mgr = ViewportManager::default();
	let (main_viewport, main_viewport_ref) = OwnedEntity::new()
		.with_debug_label("main viewport")
		.with(Viewport::new(
			&gfx,
			main_window,
			Some(main_surface),
			wgpu::SurfaceConfiguration {
				usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
				format: wgpu::TextureFormat::Bgra8Unorm,
				width: 0,
				height: 0,
				present_mode: wgpu::PresentMode::default(),
				alpha_mode: wgpu::CompositeAlphaMode::Auto,
				view_formats: Vec::new(),
			},
		))
		.with(InputManager::default())
		.with(FullScreenTexture::new(
			"depth texture",
			wgpu::TextureFormat::Depth32Float,
			wgpu::TextureUsages::RENDER_ATTACHMENT,
		))
		.split_guard();

	viewport_mgr.register(main_viewport);

	// Create engine root
	let (engine, engine_ref) = OwnedEntity::new()
		.with_debug_label("engine root")
		.with(bhv)
		.with(gfx)
		.with(viewport_mgr)
		.with(SceneManager::default())
		.split_guard();

	// Setup an initial scene
	engine
		.get_mut::<SceneManager>()
		.set_initial(make_game_scene_root(engine_ref, main_viewport_ref));

	// Show all viewports
	{
		let viewports = storage::<Viewport>();
		for (_, viewport) in engine.get::<ViewportManager>().window_map() {
			viewports.get(viewport.entity()).window().set_visible(true);
		}
	}

	// Create the handler and start the main loop
	#[derive(Debug)]
	struct MyMainLoopHandler {
		engine: OwnedEntity,
	}

	impl MainLoopHandler for MyMainLoopHandler {
		fn on_update(&mut self, main_loop: &mut MainLoop, _winit: &WinitEventProxy) {
			// Swap scenes
			let mut scene_mgr = self.engine.get_mut::<SceneManager>();
			drop(scene_mgr.swap_scenes());

			// Update the current scene
			let scene = scene_mgr.current();
			drop(scene_mgr);
			scene.get::<SceneUpdateHandler>()(scene, main_loop);

			// Reset input trackers and request redraws
			for (_, viewport) in self.engine.get::<ViewportManager>().window_map() {
				viewport.get::<Viewport>().window().request_redraw();
				viewport.get_mut::<InputManager>().end_tick();
			}
		}

		fn on_render(
			&mut self,
			_main_loop: &mut MainLoop,
			_winit: &WinitEventProxy,
			window_id: WindowId,
		) {
			let gfx = &*self.engine.get::<GfxContext>();

			// Acquire the current frame
			let Some(viewport) = self.engine.get::<ViewportManager>().get_viewport(window_id) else {
				return;
			};

			let mut frame = match viewport.get_mut::<Viewport>().present(gfx) {
				Ok(Some(frame)) => frame,
				Ok(None) => return,
				Err(err) => {
					log::error!("Failed to acquire frame: {err:?}");
					return;
				}
			};

			// Render the current scene
			let scene = self.engine.get::<SceneManager>().current();
			scene.get::<SceneRenderHandler>()(scene, viewport, &mut frame);

			// Present the frame
			frame.present();
		}

		fn on_window_input(
			&mut self,
			main_loop: &mut MainLoop,
			_winit: &WinitEventProxy,
			window_id: WindowId,
			event: WindowEvent,
		) {
			if matches!(event, WindowEvent::CloseRequested) {
				main_loop.exit();
				return;
			}

			let Some(viewport) = self.engine.get::<ViewportManager>().get_viewport(window_id) else {
				return;
			};

			viewport
				.get_mut::<InputManager>()
				.handle_window_event(&event);
		}

		fn on_device_input(
			&mut self,
			_main_loop: &mut MainLoop,
			_winit: &WinitEventProxy,
			device_id: winit::event::DeviceId,
			event: winit::event::DeviceEvent,
		) {
			for (_, viewport) in self.engine.get::<ViewportManager>().window_map() {
				viewport
					.get_mut::<InputManager>()
					.handle_device_event(device_id, &event);
			}
		}

		fn on_shutdown(self) {
			drop(self.engine);

			let leaked = bort::debug::alive_entity_count();
			if leaked > 0 {
				log::warn!(
					"Leaked {leaked} {}.",
					if leaked == 1 { "entity" } else { "entities" }
				);
			}
		}
	}

	MainLoop::start(event_loop, MyMainLoopHandler { engine });
}
