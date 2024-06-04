use std::sync::Arc;

use main_loop::{
    define_judge, feat_requires_power_pref, feat_requires_screen, InputManager, Judge,
};
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, DeviceId, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId},
};

fn main() {
    let event_loop = EventLoop::new().unwrap();
    event_loop.run_app(&mut MyApp::default()).unwrap();
}

#[derive(Default)]
struct MyApp {
    manager: InputManager,
    app: Option<MyAppInit>,
}

struct MyAppInit {
    main_window: Arc<Window>,
}

define_judge! {
    pub struct MyAppFeatures {
        _requires_screen: () => feat_requires_screen,
        _power_pref: () => {
            feat_requires_power_pref(wgpu::PowerPreference::HighPerformance)
                .map_optional(10.)
        },
    }
}

impl ApplicationHandler<()> for MyApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.app.is_none() {
            let main_window = Arc::new(
                event_loop
                    .create_window(Window::default_attributes().with_title("Hello"))
                    .unwrap(),
            );

            self.app = Some(MyAppInit { main_window });
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
