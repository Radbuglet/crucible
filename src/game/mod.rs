use std::rc::Rc;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{EventLoop, ControlFlow};
use winit::window::WindowBuilder;
use crate::core::provider::{Provider, KeyOut, ProviderExt};
use crate::core::router::GObjAncestry;
use crate::engine::WindowSizePx;
use crate::engine::input::InputTracker;
use crate::engine::gfx::{
    GfxSingletons, Viewport,
    WindowManager, RegisteredWindow, ViewportHandler,
};
use wgpu::SwapChainFrame;

pub struct OApplication {
    gfx: GfxSingletons,
    wm: WindowManager,
    inp: InputTracker,
}

impl OApplication {
    pub fn start() -> ! {
        // Set up GraphicsSingletons
        let event_loop = EventLoop::new();
        let gfx = futures::executor::block_on(
            GfxSingletons::new(wgpu::BackendBit::PRIMARY)
        ).unwrap();

        // Construct windows
        let wm = WindowManager::new();

        {
            let viewport = Viewport::new(&gfx, WindowBuilder::new()
                .with_title("Main window")
                .with_inner_size(LogicalSize::new(1920, 1080))
                .build(&event_loop).unwrap());
            wm.add(viewport, Rc::new(MyViewportHandler));
        }

        // Construct other services
        let inp = InputTracker::new();

        // Start app
        let app = OApplication { gfx, wm, inp };

        event_loop.run(move |event, _proxy, flow| {
            *flow = ControlFlow::Poll;

            let ancestry = GObjAncestry::root(&app);

            // Handle inputs
            app.fetch::<InputTracker>().handle(&event);

            // Handle windowing
            let wm = app.fetch::<WindowManager>();
            wm.handle_event(&ancestry, &event);

            if wm.viewport_map().borrow().is_empty() {
                *flow = ControlFlow::Exit;
                return;
            }
        });
    }
}

impl Provider for OApplication {
    fn get_raw<'val>(&'val self, out: &mut KeyOut<'_, 'val>) -> bool {
        self.gfx.get_raw(out) ||
            self.inp.get_raw(out) ||
            self.wm.get_raw(out)
    }
}

struct MyViewportHandler;

impl Provider for MyViewportHandler {
    fn get_raw<'val>(&'val self, out: &mut KeyOut<'_, 'val>) -> bool {
        out.field::<dyn ViewportHandler>(self)
    }
}

impl ViewportHandler for MyViewportHandler {
    fn window_event(&self, ancestry: &GObjAncestry, window: &Rc<RegisteredWindow>, event: &WindowEvent) {
        if let WindowEvent::CloseRequested = event {
            ancestry.fetch::<WindowManager>().remove(window);
            return;
        }

        let input_tracker = ancestry.fetch::<InputTracker>();
        window
            .viewport().window()
            .set_title(format!("Mouse pos: {:?}", input_tracker.mouse_pos()).as_str());
    }

    fn resized(&self, _ancestry: &GObjAncestry, _window: &Rc<RegisteredWindow>, _new_size: WindowSizePx) {
        //unimplemented!()
    }

    fn redraw(&self, _ancestry: &GObjAncestry, _window: &Rc<RegisteredWindow>, _frame: SwapChainFrame) {
        //unimplemented!()
    }
}
