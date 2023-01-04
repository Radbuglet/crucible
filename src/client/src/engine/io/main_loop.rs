use std::time::{Duration, Instant};

use crucible_util::{debug::userdata::BoxedUserdata, lang::explicitly_bind::ExplicitlyBind};
use winit::{
	event::{DeviceEvent, DeviceId, Event, WindowEvent},
	event_loop::{EventLoop, EventLoopWindowTarget},
	window::WindowId,
};

// === MainLoop === //

pub type WinitUserdata = BoxedUserdata;

pub type WinitEventLoop = EventLoop<WinitUserdata>;

pub type WinitEventProxy = EventLoopWindowTarget<WinitUserdata>;

#[derive(Debug)]
pub struct MainLoop {
	last_update: Instant,
	max_ups: u32,
	exit_requested: Option<i32>,
}

impl MainLoop {
	pub fn start(
		event_loop: EventLoop<WinitUserdata>,
		handler: impl MainLoopHandler + 'static,
	) -> ! {
		// Create main loop state
		let mut main_loop = MainLoop {
			last_update: Instant::now(),
			max_ups: 60,
			exit_requested: None,
		};
		let mut handler = ExplicitlyBind::new(handler);

		// Run main loop
		event_loop.run(move |event, proxy, flow| {
			match event {
				// First, winit notifies us of a new event batch.
				Event::NewEvents(_) => {}

				// Next, it dispatches window, user, suspension/resumption, and device events.
				Event::WindowEvent { window_id, event } => {
					handler.on_window_input((&mut main_loop, proxy), window_id, event);
				}

				Event::DeviceEvent { device_id, event } => {
					handler.on_device_input((&mut main_loop, proxy), device_id, event);
				}

				Event::UserEvent(userdata) => {
					handler.on_userdata((&mut main_loop, proxy), userdata);
				}

				Event::Suspended => {
					// We don't need to handle this as a blur event because it's mobile-specific.
				}

				Event::Resumed => {
					// We don't need to handle this as a blur event because it's mobile-specific.
				}

				// Between event handling and redraws, we have the opportunity to run update logic.
				Event::MainEventsCleared => {
					let update_start = Instant::now();
					let next_update = main_loop.next_update();

					if update_start > next_update {
						// Run user-define update logic.
						// It is up to the update handler to queue up redraw requests where
						// applicable.
						handler.on_update((&mut main_loop, proxy));

						// Wait until the next update.
						main_loop.last_update = update_start;
						let next_update = main_loop.next_update();
						flow.set_wait_until(next_update);
					} else {
						// We need to wait a bit more before the next update/render cycle.
						// We will still process redraw requests if they were queued up by
						// the OS.
						//
						// Note that `MainEventsCleared` will always be called before `flow`
						// is interpreted by winit so it's fine to only set the flow here.
						flow.set_wait_until(next_update);
					}
				}

				// Now, we receive redraw requests. These can either be app-generated refresh requests
				// created by the previous event or OS generated redraws.
				Event::RedrawRequested(window_id) => {
					handler.on_render((&mut main_loop, proxy), window_id);
				}

				// This is handled after all redraw requests have cleared, even if none were queued up.
				// In other words, this is the last event to be fired until the next batch.
				Event::RedrawEventsCleared => {
					if let Some(exit_code) = main_loop.exit_requested {
						flow.set_exit_with_code(exit_code);
					}
				}

				// This is the last thing to run before our engine is torn down.
				Event::LoopDestroyed => {
					// Run the handler's destructor and drop logic.
					ExplicitlyBind::extract(&mut handler).on_shutdown();

					// (the `main_loop` is dropped later but doesn't really do much so this is the likely
					// the last real code the app will run)
					log::info!("Goodbye!");
				}
			}
		});
	}

	pub fn exit(&mut self) {
		self.exit_with_code(0);
	}

	pub fn exit_with_code(&mut self, code: i32) {
		self.exit_requested = Some(code);
	}

	pub fn exit_requested(&self) -> Option<i32> {
		self.exit_requested
	}

	pub fn set_max_ups(&mut self, max_ups: u32) {
		self.max_ups = max_ups;
	}

	pub fn max_ups(&self) -> u32 {
		self.max_ups
	}

	pub fn min_delta(&self) -> Duration {
		Duration::from_secs(1) / self.max_ups
	}

	pub fn next_update(&self) -> Instant {
		self.last_update + self.min_delta()
	}
}

pub trait MainLoopHandler: Sized {
	fn on_update(&mut self, cx: (&mut MainLoop, &WinitEventProxy));

	fn on_render(&mut self, cx: (&mut MainLoop, &WinitEventProxy), window_id: WindowId);

	fn on_userdata(&mut self, cx: (&mut MainLoop, &WinitEventProxy), event: BoxedUserdata) {
		let _cx = cx;
		let _event = event;
	}

	fn on_window_input(
		&mut self,
		cx: (&mut MainLoop, &WinitEventProxy),
		window_id: WindowId,
		event: WindowEvent,
	) {
		let _cx = cx;
		let _window_id = window_id;
		let _event = event;
	}

	fn on_device_input(
		&mut self,
		cx: (&mut MainLoop, &WinitEventProxy),
		device_id: DeviceId,
		event: DeviceEvent,
	) {
		let _cx = cx;
		let _device_id = device_id;
		let _event = event;
	}

	fn on_shutdown(self) {}
}
