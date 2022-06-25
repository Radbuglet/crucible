use std::cell::RefCell;

use anyhow::Context;
use geode::prelude::*;
use winit::{
	dpi::LogicalSize,
	event::{Event, WindowEvent},
	event_loop::{ControlFlow, EventLoop},
	window::WindowBuilder,
};

use crate::{
	game::entry::make_game_entry,
	util::{features::FeatureList, winit::WinitEventBundle},
};

use super::{
	gfx::{
		CompatQueryInfo, GfxContext, GfxFeatureDetector, GfxFeatureNeedsScreen,
		GfxFeaturePowerPreference,
	},
	input::InputTracker,
	scene::{SceneManager, SceneUpdateHandler},
	viewport::{Viewport, ViewportManager},
};

proxy_key! {
	pub struct MainLockKey of Lock;
}

pub fn main_inner() -> anyhow::Result<()> {
	// Initialize `env_logger`
	env_logger::init();

	// Create main thread lock.
	let (mut main_lock_token, main_lock) = LockToken::new("main thread");

	// Create our main session.
	let session = Session::new([&mut main_lock_token]);
	let s = &session;

	// Initialize services
	let event_loop = EventLoop::new();

	let engine_root = {
		// Create the main window for which we'll create our main surface.
		let main_window = WindowBuilder::new()
			.with_title("Crucible")
			.with_inner_size(LogicalSize::new(1920u32, 1080u32))
			.with_visible(false)
			.build(&event_loop)
			.context("failed to create main window")?;

		// Initialize a graphics context.
		let (gfx, _table, main_surface) =
			futures::executor::block_on(GfxContext::init(&main_window, &mut MyFeatureList))
				.context("failed to create graphics context")?;

		let gfx = gfx.box_obj(s);

		// Create `ViewportManager`
		let viewport_mgr = ViewportManager::default().box_obj_rw(s, main_lock);
		{
			// Acquire services
			let mut viewport_mgr_p = viewport_mgr.borrow_mut(s);
			let gfx_p = gfx.get(s);

			// Construct main viewport
			let input_mgr = InputTracker::default().box_obj_rw(s, main_lock);
			let main_viewport = Entity::new_with(s, input_mgr);

			// Register main viewport
			viewport_mgr_p.register(
				s,
				main_lock,
				gfx_p,
				main_viewport,
				main_window,
				main_surface,
			);
		}

		// Create `SceneManager`
		let scene_mgr = SceneManager::default().box_obj_rw(s, main_lock);
		scene_mgr
			.borrow_mut(s)
			.init_scene(make_game_entry(s, main_lock));

		// Create root entity
		let root = Entity::new_with(
			s,
			(
				gfx,
				viewport_mgr,
				scene_mgr,
				ExposeUsing(main_lock.box_obj(s), MainLockKey::key()),
			),
		);
		root.manually_manage()
	};

	// Start engine
	{
		let viewport_mgr_p = engine_root.borrow::<ViewportManager>(s);

		for (_, _viewport, window) in viewport_mgr_p.mounted_viewports(s) {
			window.set_visible(true);
		}
	}

	drop(session);

	event_loop.run(move |event, proxy, flow| {
		// Acquire new session
		let session = Session::new([&mut main_lock_token]);
		let s = &session;

		// Acquire root context
		let bundle = WinitEventBundle { event, proxy, flow };

		// Acquire services
		let _gfx = engine_root.get::<GfxContext>(s);

		match &bundle.event {
			// First, `NewEvents` is triggered.
			Event::NewEvents(_) => {
				// (nothing to do here yet)
			}

			// Then, window, device, and user events are triggered.
			Event::WindowEvent { event, window_id } => {
				let mut viewport_mgr = engine_root.borrow_mut::<ViewportManager>(s);

				let viewport = match viewport_mgr.get_viewport(*window_id) {
					Some(viewport) => viewport,
					None => {
						log::warn!("Received WindowEvent for unregistered viewport {window_id:?}. Ignoring.");
						return;
					}
				};

				// Handle inputs
				viewport
					.borrow_mut::<InputTracker>(s)
					.handle_window_event(event);

				// Handle close requests
				if let WindowEvent::CloseRequested = event {
					drop(viewport.borrow_mut::<Viewport>(s).unmount());
				}

				if let WindowEvent::Destroyed = event {
					// FIXME: This isn't sent on MacOS
					viewport_mgr.unregister(*window_id);
					if viewport_mgr.mounted_viewports(s).next().is_none() {
						*bundle.flow = ControlFlow::Exit;
					}
				}
			}
			Event::DeviceEvent { device_id, event } => {
				let viewport_mgr = engine_root.borrow::<ViewportManager>(s);
				for (_, viewport) in viewport_mgr.all_viewports() {
					viewport
						.borrow_mut::<InputTracker>(s)
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
					let sm = engine_root.get::<RefCell<SceneManager>>(s);

					// Swap scenes
					sm.borrow_mut().swap_scenes();

					let sm = sm.borrow();
					let current_scene = sm.current_scene();

					current_scene.get::<dyn SceneUpdateHandler>(s).on_update(
						s,
						current_scene,
						engine_root,
					);
				}

				// Dispatch per-frame viewport logic
				let viewport_mgr = engine_root.borrow::<ViewportManager>(s);
				for (_, viewport, window) in viewport_mgr.mounted_viewports(s) {
					viewport.borrow_mut::<InputTracker>(s).end_tick();
					window.request_redraw();
				}
			}

			// Redraws are processed
			Event::RedrawRequested(_window_id) => {
				// TODO
			}
			Event::RedrawEventsCleared => {}

			// This is triggered once immediately before the engine terminates.
			// All remaining destructors will run after this.
			Event::LoopDestroyed => {
				// Destroy the engine root to run remaining finalizers
				engine_root.destroy(s);

				// Then, log goodbye.
				log::info!("Goodbye!");
			}
		}
	});
}

struct MyFeatureList;

impl GfxFeatureDetector for MyFeatureList {
	type Table = ();

	fn query_compat(&mut self, info: &mut CompatQueryInfo) -> (FeatureList, Option<Self::Table>) {
		let mut feature_list = FeatureList::default();

		feature_list.import_from(GfxFeatureNeedsScreen.query_compat(info).0);
		feature_list.import_from(
			GfxFeaturePowerPreference(wgpu::PowerPreference::HighPerformance)
				.query_compat(info)
				.0,
		);

		feature_list.wrap_user_table(())
	}
}
