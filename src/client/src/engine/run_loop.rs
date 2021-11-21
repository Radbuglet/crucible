use crate::engine::context::GfxContext;
use crate::engine::viewport::ViewportManager;
use crucible_core::foundation::prelude::*;
use std::ops::Deref;
use std::time::{Duration, Instant};
use winit::event::{DeviceEvent, DeviceId, Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop, EventLoopWindowTarget};

// === Main logic === //

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum RunLoopCommand {
	Shutdown,
}

pub type DepGuard<'a> = RwGuard<(&'a mut ViewportManager, &'a mut RunLoopStatTracker)>;

pub trait RunLoopHandler {
	type Engine;

	fn tick(
		&mut self,
		ev_pusher: &mut EventPusherPoll<RunLoopCommand>,
		engine: &Self::Engine,
		event_loop: &EventLoopWindowTarget<()>,
		dep_guard: DepGuard,
	);

	fn draw(
		&mut self,
		ev_pusher: &mut EventPusherPoll<RunLoopCommand>,
		engine: &Self::Engine,
		event_loop: &EventLoopWindowTarget<()>,
		dep_guard: DepGuard,
		window: Entity,
		frame: &wgpu::SurfaceTexture,
	);

	fn window_input(
		&mut self,
		ev_pusher: &mut EventPusherPoll<RunLoopCommand>,
		engine: &Self::Engine,
		event_loop: &EventLoopWindowTarget<()>,
		dep_guard: DepGuard,
		window: Entity,
		event: &WindowEvent,
	);

	fn device_input(
		&mut self,
		ev_pusher: &mut EventPusherPoll<RunLoopCommand>,
		engine: &Self::Engine,
		event_loop: &EventLoopWindowTarget<()>,
		dep_guard: DepGuard,
		device_id: DeviceId,
		event: &DeviceEvent,
	);

	fn goodbye(&mut self, engine: &Self::Engine, dep_guard: DepGuard);
}

pub fn start_run_loop<P, H>(event_loop: EventLoop<()>, engine: H::Engine, mut handler: H) -> !
where
	H: 'static + RunLoopHandler,
	H::Engine: Deref<Target = P> + Send + Clone,
	P: Provider,
{
	debug_assert!(
		engine.has_many::<(&GfxContext, &RwLock<RunLoopStatTracker>, &RwLock<ViewportManager>)>(),
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
			stats: &mut RunLoopStatTracker,
		);

		// Process event
		let mut ev_pusher = EventPusherPoll::new();
		match &event {
			// Loop idle handling
			Event::MainEventsCleared => {
				if stats.until_next_tick().is_none() {
					stats.begin_tick();

					// Update
					handler.tick(&mut ev_pusher, &engine, proxy, dep_guard);

					// Render
					for e_window in vm.get_entities() {
						vm.get_viewport(e_window).unwrap().window().request_redraw();
					}

					stats.end_tick();
				}

				*flow = ControlFlow::WaitUntil(stats.next_tick());
			}
			// Redraw request handling
			Event::RedrawRequested(window_id) => {
				let e_window = vm.get_entity(*window_id);
				if let Some(e_window) = e_window {
					let viewport = vm.get_viewport_mut(e_window).unwrap();

					if let Some(frame) = viewport.redraw(gfx) {
						log::trace!("Drawing to viewport {:?}", e_window);
						handler.draw(&mut ev_pusher, &engine, proxy, dep_guard, e_window, &frame);
						frame.present();
					}
				}
			}
			// IO event handling
			Event::WindowEvent { window_id, event } => {
				let e_window = vm.get_entity(*window_id);
				if let Some(e_window) = e_window {
					handler.window_input(
						&mut ev_pusher,
						&engine,
						proxy,
						dep_guard,
						e_window,
						event,
					);
				}
			}
			Event::DeviceEvent { device_id, event } => {
				handler.device_input(&mut ev_pusher, &engine, proxy, dep_guard, *device_id, event);
			}
			Event::LoopDestroyed => {
				handler.goodbye(&engine, dep_guard);
				log::info!("Goodbye!");
				return;
			}
			_ => {}
		}

		// Handle user events
		for ev in ev_pusher.drain() {
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
pub struct RunLoopStatTracker {
	// Config
	max_tps: u32,
	tick_wait_period: Duration,

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

impl RunLoopStatTracker {
	pub fn start(max_tps: u32) -> Self {
		let now = Instant::now();
		Self {
			max_tps,
			tick_wait_period: Self::tps_to_wait_period(max_tps),
			last_tick_start: now,
			last_sec: now,
			accum_tps: 0,
			accum_mspt: Duration::ZERO,
			stat_delta: None,
			stat_tps: None,
			stat_mspt: None,
		}
	}

	pub fn begin_tick(&mut self) {
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

	pub fn end_tick(&mut self) -> Option<Duration> {
		let now = Instant::now();
		self.accum_mspt += now - self.last_tick_start;
		self.until_next_tick_inner(now)
	}

	pub fn next_tick(&self) -> Instant {
		self.last_tick_start + self.tick_wait_period
	}

	fn until_next_tick_inner(&self, now: Instant) -> Option<Duration> {
		let next_tick = self.next_tick();
		if now > next_tick {
			None // We're late
		} else {
			Some(next_tick - now)
		}
	}

	pub fn until_next_tick(&self) -> Option<Duration> {
		self.until_next_tick_inner(Instant::now())
	}

	pub fn last_tick_start(&self) -> Instant {
		self.last_tick_start
	}

	pub fn last_sec(&self) -> Instant {
		self.last_sec
	}

	pub fn delta(&self) -> Option<Duration> {
		self.stat_delta
	}

	pub fn tps(&self) -> Option<u32> {
		self.stat_tps
	}

	pub fn mspt(&self) -> Option<Duration> {
		self.stat_mspt
	}

	pub fn set_max_tps(&mut self, max_tps: u32) {
		self.max_tps = max_tps;
		self.tick_wait_period = Self::tps_to_wait_period(max_tps);
	}

	pub fn max_tps(&self) -> u32 {
		self.max_tps
	}

	pub fn reset(&mut self) {
		*self = Self::start(self.max_tps);
	}

	fn tps_to_wait_period(tps: u32) -> Duration {
		if tps == 0 {
			Duration::ZERO
		} else {
			Duration::from_secs_f32(1. / tps as f32)
		}
	}
}
