use anyhow::Context;
use geode::{Entity, OwnedEntity};
use winit::{
	dpi::LogicalSize,
	event::WindowEvent,
	event_loop::EventLoopBuilder,
	window::{WindowBuilder, WindowId},
};

use crate::{
	engine::{
		gfx::texture::FullScreenTexture,
		io::main_loop::{MainLoop, MainLoopHandler, WinitEventProxy},
		scene::{SceneRenderHandler, SceneUpdateHandler},
	},
	game::entry::make_game_scene,
};

use super::{
	assets::AssetManager,
	io::{
		gfx::{GfxContext, GfxFeatureNeedsScreen},
		input::InputManager,
		viewport::{Viewport, ViewportManager},
	},
	scene::SceneManager,
};

pub fn main() -> anyhow::Result<()> {
	// Create main window
	let event_loop = EventLoopBuilder::with_user_event().build();
	let main_window = WindowBuilder::default()
		.with_title("Crucible")
		.with_inner_size(LogicalSize::new(1920, 1080))
		.with_visible(false)
		.build(&event_loop)
		.context("failed to create main window")?;

	// Initialize graphics
	let (gfx, (), main_surface) =
		futures::executor::block_on(GfxContext::init(&main_window, &mut GfxFeatureNeedsScreen))
			.context("failed to initialize a graphics adapter")?;

	// Create primary viewport
	let main_viewport = Entity::new()
		.with_debug_label("main viewport")
		.with(Viewport::new(
			&gfx,
			main_window,
			Some(main_surface),
			wgpu::SurfaceConfiguration {
				usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
				format: wgpu::TextureFormat::Bgra8UnormSrgb,
				width: 0,
				height: 0,
				present_mode: wgpu::PresentMode::default(),
				alpha_mode: wgpu::CompositeAlphaMode::Opaque,
				view_formats: Vec::new(),
			},
		))
		.with(InputManager::default())
		.with(FullScreenTexture::new(
			"depth texture",
			wgpu::TextureFormat::Depth32Float,
			wgpu::TextureUsages::RENDER_ATTACHMENT,
		));

	// Create engine
	let (engine, engine_ref) = Entity::new()
		.with_debug_label("engine root")
		.with(gfx)
		.with(SceneManager::default())
		.with(ViewportManager::default())
		.with(AssetManager::default())
		.split_guard();

	// Register main viewport
	let main_viewport_ref = main_viewport.entity();
	engine.get_mut::<ViewportManager>().register(main_viewport);

	// Setup initial scene
	engine
		.get_mut::<SceneManager>()
		.set_initial(make_game_scene(engine_ref, main_viewport_ref));

	// Show all viewports
	for (_, viewport) in engine.get::<ViewportManager>().window_map() {
		viewport.get::<Viewport>().window().set_visible(true);
	}

	// Start main loop
	#[derive(Debug)]
	struct EngineRootHandler {
		engine: OwnedEntity,
	}

	impl MainLoopHandler for EngineRootHandler {
		fn on_update(&mut self, _main_loop: &mut MainLoop, _winit: &WinitEventProxy) {
			let mut sm = self.engine.get_mut::<SceneManager>();

			// Swap scenes
			drop(sm.swap_scenes());

			// Update the current scene
			let scene = sm.current();
			drop(sm); // (allow the scene handler to access our `SceneManager`)
			scene.get::<SceneUpdateHandler>()(scene);

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

			let mut frame = match viewport.get_mut::<Viewport>().present(&gfx) {
				Ok(Some(frame)) => frame,
				Ok(None) => return,
				Err(err) => {
					log::error!("Failed to acquire frame: {err:?}");
					return;
				}
			};

			// Render the current scene
			let scene = self.engine.get::<SceneManager>().current();
			scene.get::<SceneRenderHandler>()(scene, &mut frame);

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

			let leaked = geode::debug::alive_entity_count();
			if leaked > 0 {
				log::warn!(
					"Leaked {leaked} {}.",
					if leaked == 1 { "entity" } else { "entities" }
				);
			}
		}
	}

	MainLoop::start(event_loop, EngineRootHandler { engine });
}
