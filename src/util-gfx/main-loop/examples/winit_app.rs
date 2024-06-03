use std::time::Instant;

use main_loop::{InputManager, LimitedRate, TickResult};
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, DeviceId, StartCause, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};

fn main() {
    let event_loop = EventLoop::new().unwrap();
    event_loop.run_app(&mut MyApp::default()).unwrap();
}

struct MyApp {
    manager: InputManager,
    rate_limiter: LimitedRate,
    main_window: Option<Window>,
}

impl Default for MyApp {
    fn default() -> Self {
        Self {
            manager: InputManager::default(),
            rate_limiter: LimitedRate::new(10.),
            main_window: None,
        }
    }
}

impl ApplicationHandler<()> for MyApp {
    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        let tick = self.rate_limiter.tick(Instant::now());

        match tick {
            TickResult::Tick(()) => {
                let Some(main_window) = self.main_window.as_ref() else {
                    return;
                };

                if self
                    .manager
                    .window(main_window.id())
                    .logical_key(Key::Named(NamedKey::Escape))
                    .recently_pressed()
                {
                    event_loop.exit();
                }

                self.manager.end_tick();
            }
            TickResult::Sleep(until) => {
                event_loop.set_control_flow(ControlFlow::WaitUntil(until));
            }
        }
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.main_window.is_none() {
            let window = event_loop
                .create_window(Window::default_attributes().with_title("Hello"))
                .unwrap();

            self.main_window = Some(window);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        self.manager.process_window_event(window_id, &event);
    }

    fn device_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        device_id: DeviceId,
        event: DeviceEvent,
    ) {
        self.manager.process_device_event(device_id, &event);
    }
}
