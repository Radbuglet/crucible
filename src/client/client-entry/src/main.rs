use std::process;

use anyhow::Context;
use main_loop::run_app_with_init;
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, DeviceId, StartCause, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId},
};

fn main() {
    color_backtrace::install();
    tracing_subscriber::fmt::init();

    tracing::info!("Hello!");

    if let Err(err) = main_inner() {
        tracing::error!("Fatal error ocurred during engine startup:\n{err:?}");
        process::exit(1);
    }

    tracing::info!("Goodbye!");
}

fn main_inner() -> anyhow::Result<()> {
    let event_loop = EventLoop::new().context("failed to create event loop")?;

    run_app_with_init(event_loop, |event_loop| {
        let main_window = event_loop.create_window(
            Window::default_attributes()
                .with_title("Crucible")
                .with_blur(true),
        )?;

        Ok(WinitApp { main_window })
    })
}

struct WinitApp {
    main_window: Window,
}

impl ApplicationHandler for WinitApp {
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
        device_id: DeviceId,
        event: DeviceEvent,
    ) {
    }
}
