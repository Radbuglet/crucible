use crate::engine::context::GfxContext;
use crate::engine::viewport::ViewportManager;
use crucible_core::foundation::prelude::*;
use std::ops::Deref;
use winit::event::{DeviceEvent, DeviceId, Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop, EventLoopWindowTarget};

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum RunLoopCommand {
	Shutdown,
}

pub trait RunLoopHandler {
	type Engine;

	fn tick(
		&mut self,
		ev_pusher: &mut EventPusherPoll<RunLoopCommand>,
		engine: &Self::Engine,
		event_loop: &EventLoopWindowTarget<()>,
		vm_guard: RwGuardMut<ViewportManager>,
	);

	fn draw(
		&mut self,
		ev_pusher: &mut EventPusherPoll<RunLoopCommand>,
		engine: &Self::Engine,
		event_loop: &EventLoopWindowTarget<()>,
		vm_guard: RwGuardMut<ViewportManager>,
		window: Entity,
		frame: &wgpu::SurfaceTexture,
	);

	fn window_input(
		&mut self,
		ev_pusher: &mut EventPusherPoll<RunLoopCommand>,
		engine: &Self::Engine,
		event_loop: &EventLoopWindowTarget<()>,
		vm_guard: RwGuardMut<ViewportManager>,
		window: Entity,
		event: &WindowEvent,
	);

	fn device_input(
		&mut self,
		ev_pusher: &mut EventPusherPoll<RunLoopCommand>,
		engine: &Self::Engine,
		event_loop: &EventLoopWindowTarget<()>,
		vm_guard: RwGuardMut<ViewportManager>,
		device_id: DeviceId,
		event: &DeviceEvent,
	);

	fn goodbye(&mut self, engine: &Self::Engine, vm_guard: RwGuardMut<ViewportManager>);
}

// TODO: Improve scheduling
// FIXME: Zero-sized windows break everything!
pub fn start_run_loop<P, H>(event_loop: EventLoop<()>, engine: H::Engine, mut handler: H) -> !
where
	H: 'static + RunLoopHandler,
	H::Engine: Deref<Target = P> + Send + Clone,
	P: Provider,
{
	debug_assert!(
		engine.has_many::<(&GfxContext, &RwLock<ViewportManager>)>(),
		"`start_run_loop` requires a `GfxContext` and an `RwLock<ViewportManager>`!"
	);

	// TODO: Really improve scheduling!
	// ^ We should be able to formalize how many in-flight frames are allowed and make sure that all
	// map requests can be resolved immediately once the user is asked to render.
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
		let vm_guard = RwGuardMut::lock_now(engine.get());
		let vm: &mut ViewportManager = vm_guard.get();

		// Process event
		let mut ev_pusher = EventPusherPoll::new();
		match &event {
			Event::WindowEvent { window_id, event } => {
				let e_window = vm.get_entity(*window_id);
				if let Some(e_window) = e_window {
					handler.window_input(&mut ev_pusher, &engine, proxy, vm_guard, e_window, event);
				}
			}
			Event::DeviceEvent { device_id, event } => {
				handler.device_input(&mut ev_pusher, &engine, proxy, vm_guard, *device_id, event);
			}
			Event::RedrawRequested(window_id) => {
				let e_window = vm.get_entity(*window_id);
				if let Some(e_window) = e_window {
					let viewport = vm.get_viewport_mut(e_window).unwrap();

					if let Some(frame) = viewport.redraw(gfx) {
						log::trace!("Drawing to viewport {:?}", e_window);
						handler.draw(&mut ev_pusher, &engine, proxy, vm_guard, e_window, &frame);
						frame.present();
					} else {
						log::warn!("Failed to acquire frame to draw to viewport {:?}", e_window);
					}
				}
			}
			Event::MainEventsCleared => {
				log::trace!("Dispatching new round of redraw requests.");
				for e_window in vm.get_entities() {
					vm.get_viewport(e_window).unwrap().window().request_redraw();
				}
			}
			Event::LoopDestroyed => {
				handler.goodbye(&engine, vm_guard);
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
