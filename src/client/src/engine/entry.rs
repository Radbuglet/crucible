use crate::engine::gfx::{
	CompatQueryInfo, GfxContext, GfxFeatureDetector, GfxFeatureNeedsScreen,
	GfxFeaturePowerPreference,
};
use crate::engine::input::InputTracker;
use crate::engine::scene::{SceneManager, UpdateHandler};
use crate::engine::viewport::{Viewport, ViewportManager, ViewportRenderer};
use crate::game::entry::make_game_scene;
use crate::util::features::FeatureList;
use crate::util::winit::{WinitEventBundle, WinitUserdata};
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
		for (_, viewport_obj) in vm.mounted_viewports() {
			viewport_obj
				.borrow::<Viewport>()
				.window()
				.unwrap()
				.set_visible(true);
		}
	});

	// We wrap the root in an optional so we can force the destructors to run at a given time.
	let mut root = Some(root);

	event_loop.run(move |event, proxy, flow| {
		// Acquire root context
		let bundle = WinitEventBundle { event, proxy, flow };
		let root_ref = root.as_ref().unwrap();
		let cx = ObjCx::new(root_ref);

		// Acquire services
		let gfx = root_ref.get::<GfxContext>();

		match &bundle.event {
			// First, `NewEvents` is triggered.
			Event::NewEvents(_) => {}

			// Then, window, device, and user events are triggered.
			Event::WindowEvent { event, window_id } => {
				let mut viewport_mgr = root_ref.borrow_mut::<ViewportManager>();
				let viewport = match viewport_mgr.get_viewport(*window_id) {
					Some(viewport) => viewport,
					None => {
						log::warn!("Received WindowEvent for unregistered viewport {window_id:?}. Ignoring.");
						return;
					}
				};

				// Handle inputs
				viewport
					.borrow_mut::<InputTracker>()
					.handle_window_event(event);

				// Handle close requests
				if let WindowEvent::CloseRequested = event {
					drop(viewport.borrow_mut::<Viewport>().unmount());
				}

				if let WindowEvent::Destroyed = event {
					viewport_mgr.unregister(*window_id);
					if viewport_mgr.mounted_viewports().next().is_none() {
						*bundle.flow = ControlFlow::Exit;
					}
				}
			}
			Event::DeviceEvent { device_id, event } => {
				let viewport_mgr = root_ref.borrow::<ViewportManager>();
				for (_, viewport) in viewport_mgr.all_viewports() {
					viewport
						.borrow_mut::<InputTracker>()
						.handle_device_event(*device_id, event);
				}
			}
			Event::UserEvent(_) => {}

			// These are also technically window events
			Event::Suspended => {}
			Event::Resumed => {}

			// After all user events have been triggered, this event is triggered.
			Event::MainEventsCleared => {
				// TODO: This logic is kinda nonsense.

				// Handle scene manager update logic if needed.
				{
					let sm = root_ref.get::<ARefCell<SceneManager>>();
					sm.borrow_mut().swap_scenes();

					let sm = sm.borrow();
					let current_scene = sm.current_scene();
					current_scene.get::<dyn UpdateHandler>().on_update(&cx);
				}

				// Dispatch per-frame viewport logic
				let viewport_mgr = root_ref.borrow::<ViewportManager>();
				for (_, viewport) in viewport_mgr.mounted_viewports() {
					viewport.borrow_mut::<InputTracker>().end_tick();
					viewport
						.borrow_mut::<Viewport>()
						.window()
						.unwrap()
						.request_redraw();
				}
			}

			// Redraws are processed
			Event::RedrawRequested(window_id) => {
				let viewport_mgr = root_ref.borrow::<ViewportManager>();
				let viewport = match viewport_mgr.get_viewport(*window_id) {
					Some(viewport) => viewport,
					None => {
						log::warn!("Received RedrawRequested for unregistered viewport {window_id:?}. Ignoring.");
						return;
					}
				};

				let frame = match viewport.borrow_mut::<Viewport>().render(gfx).unwrap() {
					Some(frame) => frame,
					None => return,
				};

				viewport
					.get::<dyn ViewportRenderer>()
					.on_viewport_render(&cx, frame);
			}
			Event::RedrawEventsCleared => {}

			// This is triggered once immediately before the engine terminates.
			// All remaining destructors will run after this.
			Event::LoopDestroyed => {
				// Destroy the engine root to run remaining finalizers
				root = None;

				// Then, log goodbye.
				log::info!("Goodbye!");
			}
		}
	});
}

async fn make_engine_root(event_loop: &EventLoop<WinitUserdata>) -> anyhow::Result<Obj> {
	let mut root = Obj::labeled("engine root");

	// Create graphics subsystem
	{
		// Create context
		let main_window = WindowBuilder::new()
			.with_title("Crucible")
			.with_inner_size(LogicalSize::new(1920u32, 1080u32))
			.with_visible(false)
			.build(event_loop)
			.context("failed to create main window")?;

		let (gfx, _gfx_features, main_swapchain) =
			GfxContext::init(&main_window, &mut CustomFeatureListValidator)
				.await
				.context("failed to create graphics context")?;

		root.add(gfx);

		// Setup viewport manager
		let gfx = root.get::<GfxContext>();
		let mut vm = ViewportManager::default();

		let mut viewport = Obj::labeled("main viewport");
		viewport.add_alias(
			|cx: &ObjCx, frame: wgpu::SurfaceTexture| {
				cx.borrow::<SceneManager>()
					.current_scene()
					.get::<dyn ViewportRenderer>()
					// TODO: Adjust `cx`
					.on_viewport_render(cx, frame)
			},
			typed_key::<dyn ViewportRenderer>(),
		);
		viewport.add_rw(InputTracker::default());
		vm.register(gfx, viewport, main_window, main_swapchain);

		root.add_rw(vm);
	};

	// Register game subsystems
	{
		let mut sm = SceneManager::default();
		sm.init_scene(make_game_scene());
		root.add_rw(sm);
	}

	Ok(root)
}

struct CustomFeatureListValidator;

impl GfxFeatureDetector for CustomFeatureListValidator {
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
