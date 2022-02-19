use crate::engine::context::GfxContext;
use crate::engine::viewport::ViewportManager;
use crucible_core::foundation::prelude::*;
use std::collections::VecDeque;
use std::ops::Deref;
use std::time::{Duration, Instant};
use winit::event::{DeviceEvent, DeviceId, Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop, EventLoopWindowTarget};

// === Main logic === //

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum RunLoopCommand {
	Shutdown,
}

pub type DepGuard<'a> = RwGuard<(&'a mut ViewportManager, &'a mut RunLoopTiming)>;

pub trait RunLoopHandler {
	fn tick(
		&self,
		on_loop_ev: &mut VecDeque<RunLoopCommand>,
		event_loop: &EventLoopWindowTarget<()>,
		dep_guard: DepGuard,
	);

	fn draw(
		&self,
		on_loop_ev: &mut VecDeque<RunLoopCommand>,
		event_loop: &EventLoopWindowTarget<()>,
		dep_guard: DepGuard,
		window: Entity,
		frame: &wgpu::SurfaceTexture,
	);

	fn window_input(
		&self,
		on_loop_ev: &mut VecDeque<RunLoopCommand>,
		event_loop: &EventLoopWindowTarget<()>,
		dep_guard: DepGuard,
		window: Entity,
		event: &WindowEvent,
	);

	fn device_input(
		&self,
		on_loop_ev: &mut VecDeque<RunLoopCommand>,
		event_loop: &EventLoopWindowTarget<()>,
		dep_guard: DepGuard,
		device_id: DeviceId,
		event: &DeviceEvent,
	);

	fn goodbye(&self, dep_guard: DepGuard);
}

pub fn start_run_loop<S>(event_loop: EventLoop<()>, engine: S) -> !
where
	S: 'static + Send + Clone + Deref + RunLoopHandler,
	S::Target: Provider,
{
	debug_assert!(
		engine.has_many::<(&GfxContext, &RwLock<RunLoopTiming>, &RwLock<ViewportManager>)>(),
		"`start_run_loop` requires a `GfxContext`, an `RwLock<RunLoopStatTracker>`, and an `RwLock<ViewportManager>`!"
	);

	// Bind polling thread.
	// FIXME: This is taking up _way_ too much CPU time.
	{
		let engine = engine.clone();
		std::thread::spawn(move || loop {
			let gfx: &GfxContext = engine.get();
			gfx.device.poll(wgpu::Maintain::Wait);
		});
	}

	event_loop.run(move |event, proxy, flow| {
		// Get dependencies
		let gfx: &GfxContext = engine.get();
		lock_many_now!(
			engine.get_many() => dep_guard,
			vm: &mut ViewportManager,
			timings: &mut RunLoopTiming,
		);

		// Process event
		let mut on_loop_ev = VecDeque::new();
		match &event {
			// Loop idle handling
			Event::MainEventsCleared => {
				if timings.until_next_tick().is_none() {
					// Update
					timings.begin_tick();
					engine.tick(&mut on_loop_ev, proxy, dep_guard);
					timings.end_tick();

					// Render
					for e_window in vm.get_entities() {
						vm.get_viewport(e_window).unwrap().window().request_redraw();
					}
				}

				let now = Instant::now();
				if let Some(until) = timings.next_tick().checked_duration_since(now) {
					// TODO: Determine sleep overhead at runtime.
					// Take 1/3rd of the wait time.
					let until = until / 3;

					// It the wait is above 4ms, perform it.
					if until > Duration::from_millis(4) {
						*flow = ControlFlow::WaitUntil(now + until);
					}
				}
			}
			// Redraw request handling
			Event::RedrawRequested(window_id) => {
				let e_window = vm.get_entity(*window_id);
				if let Some(e_window) = e_window {
					let mut viewport = vm.get_viewport_mut(e_window).unwrap();

					if let Some(frame) = viewport.redraw(gfx) {
						log::trace!("Drawing to viewport {:?}", e_window);
						timings.render.begin_period();
						engine.draw(&mut on_loop_ev, proxy, dep_guard, e_window, &frame);
						timings.render.end_period();
						frame.present();
					}
				}
			}
			// IO event handling
			Event::WindowEvent { window_id, event } => {
				let e_window = vm.get_entity(*window_id);
				if let Some(e_window) = e_window {
					engine.window_input(&mut on_loop_ev, proxy, dep_guard, e_window, event);
				}
			}
			Event::DeviceEvent { device_id, event } => {
				engine.device_input(&mut on_loop_ev, proxy, dep_guard, *device_id, event);
			}
			Event::LoopDestroyed => {
				engine.goodbye(dep_guard);
				log::info!("Goodbye!");
				return;
			}
			_ => {}
		}

		// Handle user events
		for ev in on_loop_ev.drain(..) {
			match ev {
				RunLoopCommand::Shutdown => {
					log::info!("Shutdown requested.");
					*flow = ControlFlow::Exit;
				}
			}
		}
	});
}

// === Stat tracking === //

#[derive(Debug, Clone)]
pub struct RunLoopTiming {
	// Config
	max_tps: u32,
	tick_wait_period: Duration,

	// Trackers
	pub update: RunLoopEventTracker,
	pub render: RunLoopEventTracker,
}

impl RunLoopTiming {
	pub fn start(max_tps: u32) -> Self {
		Self {
			max_tps,
			tick_wait_period: Self::tps_to_wait_period(max_tps),

			update: RunLoopEventTracker::start(),
			render: RunLoopEventTracker::start(),
		}
	}

	//> Fps limiting
	pub fn set_max_tps(&mut self, max_tps: u32) {
		self.max_tps = max_tps;
		self.tick_wait_period = Self::tps_to_wait_period(max_tps);
	}

	pub fn max_tps(&self) -> u32 {
		self.max_tps
	}

	pub fn next_tick(&self) -> Instant {
		self.last_tick() + self.tick_wait_period
	}

	pub fn until_next_tick(&self) -> Option<Duration> {
		self.next_tick().checked_duration_since(Instant::now())
	}

	fn tps_to_wait_period(tps: u32) -> Duration {
		if tps == 0 {
			Duration::ZERO
		} else {
			Duration::from_secs_f32(1. / tps as f32)
		}
	}

	//> Update forwards
	pub fn begin_tick(&mut self) {
		self.update.begin_period();
	}

	pub fn end_tick(&mut self) {
		self.update.begin_period();
	}

	pub fn delta(&self) -> Option<Duration> {
		self.update.delta()
	}

	pub fn delta_secs(&self) -> f32 {
		self.update.delta_secs()
	}

	pub fn tps(&self) -> Option<u32> {
		self.update.tps()
	}

	pub fn mspt(&self) -> Option<Duration> {
		self.update.mspt()
	}

	pub fn last_tick(&self) -> Instant {
		self.update.last_period_start()
	}

	//> Render methods
	pub fn begin_frame(&mut self) {
		self.render.begin_period();
	}

	pub fn end_frame(&mut self) {
		self.render.begin_period();
	}

	pub fn fps(&self) -> Option<u32> {
		self.render.tps()
	}

	pub fn mspf(&self) -> Option<Duration> {
		self.render.mspt()
	}

	//> General
	pub fn last_second(&self) -> Instant {
		self.update.last_sec()
	}

	pub fn reset(&mut self) {
		self.update.reset();
		self.render.reset();
	}
}

#[derive(Debug, Clone)]
pub struct RunLoopEventTracker {
	// Period tracking
	last_tick_start: Instant,
	last_sec: Instant,
	accum_tps: u32,
	accum_mspt: Duration,

	// Stats
	stat_delta: Option<Duration>,
	stat_tps: Option<u32>,
	stat_mspt: Option<Duration>,
}

impl RunLoopEventTracker {
	pub fn start() -> Self {
		let now = Instant::now();
		Self {
			last_tick_start: now,
			last_sec: now,
			accum_tps: 0,
			accum_mspt: Duration::ZERO,
			stat_delta: None,
			stat_tps: None,
			stat_mspt: None,
		}
	}

	pub fn begin_period(&mut self) {
		// Update delta
		let now = Instant::now();
		self.stat_delta = Some(now - self.last_tick_start);
		self.last_tick_start = now;

		// Update stats
		self.accum_tps += 1;
		if now - self.last_sec >= Duration::SECOND {
			self.stat_tps = Some(self.accum_tps);
			self.stat_mspt = if self.accum_tps > 0 {
				Some(self.accum_mspt / self.accum_tps)
			} else {
				None
			};

			self.accum_tps = 0;
			self.accum_mspt = Duration::ZERO;
			self.last_sec = now;
		}
	}

	pub fn end_period(&mut self) {
		let now = Instant::now();
		self.accum_mspt += now - self.last_tick_start;
	}

	pub fn last_period_start(&self) -> Instant {
		self.last_tick_start
	}

	pub fn last_sec(&self) -> Instant {
		self.last_sec
	}

	pub fn delta(&self) -> Option<Duration> {
		self.stat_delta
	}

	pub fn delta_secs(&self) -> f32 {
		self.delta().map_or(1., |time| time.as_secs_f32())
	}

	pub fn tps(&self) -> Option<u32> {
		self.stat_tps
	}

	pub fn mspt(&self) -> Option<Duration> {
		self.stat_mspt
	}

	pub fn reset(&mut self) {
		*self = Self::start();
	}
}