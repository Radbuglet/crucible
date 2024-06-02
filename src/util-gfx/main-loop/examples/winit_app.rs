use winit::{
    application::ApplicationHandler,
    event::{StartCause, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::WindowId,
};

fn main() {
    let event_loop = EventLoop::new().unwrap();
    event_loop.run_app(&mut MyApp).unwrap();
}

struct MyApp;

impl ApplicationHandler<()> for MyApp {
    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {}

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {}

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
    }

    fn device_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
    }
}
