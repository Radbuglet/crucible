use anyhow::Context;
use bort::{
	alias, proc, proc_collection, saddle_delegate, storage, BehaviorRegistry, Entity, OwnedEntity,
	RootCollectionCallToken,
};
use crucible_foundation_client::engine::{
	assets::AssetManager,
	gfx::texture::FullScreenTexture,
	io::{
		gfx::{
			feat_requires_power_pref, feat_requires_screen, CompatQueryInfo, GfxContext, Judgement,
		},
		input::InputManager,
		main_loop::{MainLoop, MainLoopHandler, WinitEventProxy, WinitUserdata},
		viewport::{Viewport, ViewportManager},
	},
	scene::SceneManager,
};
use winit::{
	dpi::LogicalSize,
	event::WindowEvent,
	event_loop::{EventLoop, EventLoopBuilder},
	window::{WindowBuilder, WindowId},
};

use crate::game::systems::entry::spawn_game_scene_root;

// === Behaviors === //

proc_collection! {
	pub EngineEntryBehavior;
	pub SceneInitBehavior;
}

saddle_delegate! {
	pub fn SceneUpdateHandler(me: Entity, main_loop: &mut MainLoop)
}

saddle_delegate! {
	pub fn SceneRenderHandler(me: Entity, viewport: Entity, frame: &mut wgpu::SurfaceTexture)
}

// === Entry === //

alias! {
	let bhv: BehaviorRegistry;
	let gfx: GfxContext;
	let scene_mgr: SceneManager;
	let viewport_mgr: ViewportManager;
}

pub fn main_inner() -> anyhow::Result<()> {
	// Create the behavior registry
	let mut call_cx = RootCollectionCallToken::acquire();
	let bhv = BehaviorRegistry::new().with_many(crate::game::register);

	// Initialize the engine
	proc! {
		as EngineEntryBehavior[call_cx] do
		(_cx: [], _call_cx: []) {
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
						.with_sub(feat_requires_screen(info).0)
						.with_sub(
							feat_requires_power_pref(wgpu::PowerPreference::HighPerformance)(info)
								.0
								.make_soft_error(1.0),
						)
						.with_table(())
				},
			))
			.context("failed to create graphics device")?;
		}
		(cx: [ref Viewport], _call_cx: []) {
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

			viewport_mgr.register(cx, main_viewport);
		}
		(cx: [mut SceneManager], call_cx: [SceneInitBehavior]) {
			// Create engine root
			let (engine, engine_ref) = OwnedEntity::new()
				.with_debug_label("engine root")
				.with(bhv)
				.with(gfx)
				.with(viewport_mgr)
				.with(AssetManager::default())
				.with(SceneManager::default())
				.split_guard();

			// Setup an initial scene
			engine
				.get_mut_s::<SceneManager>(cx)
				.set_initial(spawn_game_scene_root(
					call_cx,
					engine_ref,
					main_viewport_ref,
				));
		}
		(cx: [], _call_cx: [], ref viewport_mgr = engine) {
			// Show all viewports
			{
				let viewports = storage::<Viewport>();
				for (_, viewport) in viewport_mgr.window_map() {
					viewports.get(viewport.entity()).window().set_visible(true);
				}
			}
		}
	}

	drop(call_cx);

	// Create the handler and start the main loop
	#[derive(Debug)]
	struct MyMainLoopHandler {
		engine: OwnedEntity,
	}

	impl MainLoopHandler for MyMainLoopHandler {
		fn on_update(&mut self, main_loop: &mut MainLoop, _winit: &WinitEventProxy) {
			let mut call_cx = RootCollectionCallToken::acquire();

			proc! {
				as EngineEntryBehavior[call_cx] do
				(
					cx: [ref SceneUpdateHandler],
					call_cx: [SceneUpdateHandler],
					mut scene_mgr = self.engine,
					ref bhv = self.engine,
				) {
					// Swap scenes
					drop(scene_mgr.swap_scenes());

					// Update the current scene
					let scene = scene_mgr.current();
					scene.get_s::<SceneUpdateHandler>(cx)(bhv.provider(), call_cx, scene, main_loop);
				}
				(
					cx: [ref Viewport, mut InputManager],
					_call_cx: [],
					ref viewport_mgr = self.engine,
				) {
					// Reset input trackers and request redraws
					for (_, viewport) in viewport_mgr.window_map() {
						viewport.get_s::<Viewport>(cx).window().request_redraw();
						viewport.get_mut_s::<InputManager>(cx).end_tick();
					}
				}
			}
		}

		fn on_render(
			&mut self,
			_main_loop: &mut MainLoop,
			_winit: &WinitEventProxy,
			window_id: WindowId,
		) {
			let mut call_cx = RootCollectionCallToken::acquire();

			proc! {
				as EngineEntryBehavior[call_cx] do
				(
					cx: [mut Viewport],
					_call_cx: [],
					ref gfx = self.engine,
					ref viewport_mgr = self.engine,
				) {
					// Acquire the current frame
					let Some(viewport) = viewport_mgr.get_viewport(window_id) else {
						return;
					};

					let mut frame = match viewport.get_mut_s::<Viewport>(cx).present(&gfx) {
						Ok(Some(frame)) => frame,
						Ok(None) => return,
						Err(err) => {
							log::error!("Failed to acquire frame: {err:?}");
							return;
						}
					};
				}
				(
					cx: [ref SceneRenderHandler],
					call_cx: [SceneRenderHandler],
					ref bhv = self.engine,
					ref scene_mgr = self.engine,
				) {
					// Render the current scene
					let scene = scene_mgr.current();
					scene.get_s::<SceneRenderHandler>(cx)(bhv.provider(), call_cx, scene, viewport, &mut frame);

					// Present the frame
					frame.present();
				}
			}
		}

		fn on_window_input(
			&mut self,
			main_loop: &mut MainLoop,
			_winit: &WinitEventProxy,
			window_id: WindowId,
			event: WindowEvent,
		) {
			let mut call_cx = RootCollectionCallToken::acquire();

			proc! {
				as EngineEntryBehavior[call_cx] do
				(
					cx: [mut Viewport, mut InputManager],
					_call_cx: [],
					ref viewport_mgr = self.engine,
				) {
					if matches!(event, WindowEvent::CloseRequested) {
						main_loop.exit();
						return;
					}

					let Some(viewport) = viewport_mgr.get_viewport(window_id) else {
						return;
					};

					viewport
						.get_mut_s::<InputManager>(cx)
						.handle_window_event(&event);
				}
			}
		}

		fn on_device_input(
			&mut self,
			_main_loop: &mut MainLoop,
			_winit: &WinitEventProxy,
			device_id: winit::event::DeviceId,
			event: winit::event::DeviceEvent,
		) {
			let mut call_cx = RootCollectionCallToken::acquire();

			proc! {
				as EngineEntryBehavior[call_cx] do
				(
					cx: [mut InputManager],
					_call_cx: [],
					ref viewport_mgr = self.engine,
				) {
					for (_, viewport) in viewport_mgr.window_map() {
						viewport
							.get_mut_s::<InputManager>(cx)
							.handle_device_event(device_id, &event);
					}
				}
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
