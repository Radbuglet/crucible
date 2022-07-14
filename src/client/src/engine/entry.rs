use geode::prelude::*;
use winit::{
	event::{Event, WindowEvent},
	event_loop::{ControlFlow, EventLoop},
};

use crate::{engine::services::viewport::ViewportRenderHandler, util::winit::WinitEventBundle};

use super::{
	root::EngineRootBundle,
	services::{input::InputTracker, scene::SceneUpdateHandler, viewport::Viewport},
};

pub fn main_inner() -> anyhow::Result<()> {
	// Initialize engine
	let (event_loop, engine_root) = {
		// Create initialization session
		let main_lock_guard = Lock::new("main thread");
		let session = LocalSessionGuard::new();
		let s = session.handle();
		s.acquire_locks([main_lock_guard.weak_copy()]);

		// Create engine root
		let event_loop = EventLoop::new();
		let engine_root = EngineRootBundle::new(s, main_lock_guard, &event_loop)?;

		// Make all viewports visible
		{
			let p_viewport_mgr = engine_root.viewport_mgr(s).borrow_mut();

			for (_, _viewport, window) in p_viewport_mgr.mounted_viewports(s) {
				window.set_visible(true);
			}
		}

		(event_loop, engine_root)
	};

	// Enter main loop
	log::info!("Entering main loop!");

	// We want to execute this destructor in the loop teardown phase instead of the unwinding phase.
	let engine_root = engine_root.manually_destruct();

	event_loop.run(move |event, proxy, flow| {
		// Create entry session
		let session = LocalSessionGuard::new();
		let s = session.handle();

		let main_lock = engine_root.main_lock(s);
		s.acquire_locks([main_lock.weak_copy()]);

		// Acquire root context
		let bundle = WinitEventBundle { event, proxy, flow };

		// Acquire services
		let p_gfx = engine_root.gfx(s);

		// Process events
		match &bundle.event {
			// First, `NewEvents` is triggered.
			Event::NewEvents(_) => {
				// (nothing to do here yet)
			}

			// Then, window, device, and user events are triggered.
			Event::WindowEvent { event, window_id } => {
				let mut viewport_mgr = engine_root.viewport_mgr(s).borrow_mut();

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
					// We unmount the surface but defer viewport deletion until the OS has finished
					// sending out its remaining events.
					drop(viewport.borrow_mut::<Viewport>(s).unmount());
				}

				if let WindowEvent::Destroyed = event {
					// FIXME: This isn't sent on MacOS
					viewport_mgr.unregister(*window_id);

					// Quit if all viewports have been destroyed.
					if viewport_mgr.mounted_viewports(s).next().is_none() {
						*bundle.flow = ControlFlow::Exit;
					}
				}
			}
			Event::DeviceEvent { device_id, event } => {
				let viewport_mgr = engine_root.viewport_mgr(s).borrow();
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
				// Handle scene manager update logic.
				let should_update = true; // TODO: Tickrate limit

				if should_update {
					let sm = engine_root.scene_mgr(s);

					// Swap scenes
					sm.borrow_mut().swap_scenes();

					// Allow scene to run update logic
					let sm = sm.borrow();
					let current_scene = sm.current_scene();
					current_scene.get::<dyn SceneUpdateHandler>(s).on_update(
						s,
						current_scene,
						engine_root.raw(),
					);
				}

				// Ensure that all viewports have a chance to render
				let viewport_mgr = engine_root.viewport_mgr(s).borrow();
				for (_, viewport, window) in viewport_mgr.mounted_viewports(s) {
					// The update handler has just processed these inputs. Clear them for the next
					// logical frame.
					if should_update {
						viewport.borrow_mut::<InputTracker>(s).end_tick();
					}

					// Right now, we request redraws every time the main loop executes a cycle.
					// TODO: Framerate limit
					window.request_redraw();
				}
			}

			// Redraws are processed
			Event::RedrawRequested(window_id) => {
				let viewport_mgr = engine_root.viewport_mgr(s).borrow();
				let viewport = match viewport_mgr.get_viewport(*window_id) {
					Some(viewport) => viewport,
					None => return,
				};

				let frame = viewport
					.borrow_mut::<Viewport>(s)
					.render(p_gfx)
					.expect("failed to get frame");

				viewport.get::<dyn ViewportRenderHandler>(s).on_render(
					frame,
					s,
					viewport,
					viewport,
					engine_root.raw(),
				);
			}
			Event::RedrawEventsCleared => {}

			// This is triggered once immediately before the engine terminates.
			// All remaining destructors will run after this.
			Event::LoopDestroyed => {
				// Destroy the engine root to run remaining finalizers
				engine_root.raw().destroy(s);

				// Then, log goodbye.
				log::info!("Goodbye!");
			}
		}
	})
}
