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
	engine::viewport::ViewportRenderHandler,
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
	pub struct MainLockKey of Owned<Lock>;
}

pub fn main_inner() -> anyhow::Result<()> {
	// Initialize engine
	let (event_loop, engine_root) = {
		// Create initialization session
		let main_lock_guard = Lock::new("main thread");
		let session = LocalSessionGuard::new();
		let s = session.handle();
		s.acquire_locks([*main_lock_guard]);

		// Create engine root
		let event_loop = EventLoop::new();
		let engine_root = make_engine_root(s, main_lock_guard, &event_loop)?;

		// Make all viewports visible
		{
			let p_viewport_mgr = engine_root.borrow::<ViewportManager>(s);

			for (_, _viewport, window) in p_viewport_mgr.mounted_viewports(s) {
				window.set_visible(true);
			}
		}

		(event_loop, engine_root)
	};

	// Enter main loop
	run_main_loop(event_loop, engine_root);

	// (this is actually unreachable)
	Ok(())
}

fn make_engine_root(
	s: Session,
	main_lock_guard: Owned<Lock>,
	event_loop: &EventLoop<()>,
) -> anyhow::Result<Owned<Entity>> {
	let engine_root_guard = Entity::new(s);
	let engine_root = *engine_root_guard;
	let main_lock = *main_lock_guard;

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

	let gfx_guard = gfx.box_obj(s);
	let gfx = *gfx_guard;

	engine_root_guard.add(s, gfx_guard);

	// Create `ViewportManager`
	let viewport_mgr = ViewportManager::default().box_obj_rw(s, main_lock);
	{
		// Acquire services
		let mut p_viewport_mgr = viewport_mgr.borrow_mut(s);
		let p_gfx = gfx.get(s);

		// Construct main viewport
		let input_mgr = InputTracker::default().box_obj_rw(s, main_lock);
		let render_handler = Obj::new(s, move |frame, s: Session, _me, viewport, engine| {
			let p_scene_mgr = engine_root.borrow::<SceneManager>(s);
			let current_scene = p_scene_mgr.current_scene();

			current_scene.get::<dyn ViewportRenderHandler>(s).on_render(
				frame,
				s,
				current_scene,
				viewport,
				engine,
			);
		})
		.to_unsized::<dyn ViewportRenderHandler>();

		let main_viewport = Entity::new_with(s, (render_handler, input_mgr));

		// Register main viewport
		p_viewport_mgr.register(
			s,
			main_lock,
			p_gfx,
			main_viewport,
			main_window,
			main_surface,
		);
	}

	// Create `SceneManager`
	let scene_mgr = SceneManager::default().box_obj_rw(s, main_lock);
	scene_mgr
		.borrow_mut(s)
		.init_scene(make_game_entry(s, *engine_root_guard, main_lock));

	// Create root entity
	engine_root_guard.add(
		s,
		(
			viewport_mgr,
			scene_mgr,
			ExposeUsing(main_lock_guard.box_obj(s), MainLockKey::key()),
		),
	);

	Ok(engine_root_guard)
}

fn run_main_loop(event_loop: EventLoop<()>, engine_root: Owned<Entity>) {
	log::info!("Entering main loop!");

	// We want to execute this destructor in the loop teardown phase instead of the unwinding phase.
	let engine_root = engine_root.manually_destruct();

	event_loop.run(move |event, proxy, flow| {
		// Create entry session
		let session = LocalSessionGuard::new();
		let s = session.handle();

		let main_lock = **engine_root.get_in(s, MainLockKey::key());
		s.acquire_locks([main_lock]);

		// Acquire root context
		let bundle = WinitEventBundle { event, proxy, flow };

		// Acquire services
		let p_gfx = engine_root.get::<GfxContext>(s);

		// Process events
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
				// Handle scene manager update logic.
				let should_update = true; // TODO: Tickrate limit

				if should_update {
					let sm = engine_root.get::<RefCell<SceneManager>>(s);

					// Swap scenes
					sm.borrow_mut().swap_scenes();

					// Allow scene to run update logic
					let sm = sm.borrow();
					let current_scene = sm.current_scene();
					current_scene.get::<dyn SceneUpdateHandler>(s).on_update(
						s,
						current_scene,
						engine_root,
					);
				}

				// Ensure that all viewports have a chance to render
				let viewport_mgr = engine_root.borrow::<ViewportManager>(s);
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
				let viewport_mgr = engine_root.borrow::<ViewportManager>(s);
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
					engine_root,
				);
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
