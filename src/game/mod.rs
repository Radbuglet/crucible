use std::rc::Rc;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{EventLoop, ControlFlow};
use winit::window::WindowBuilder;
use crate::core::game_object::{GameObject, KeyOut, GameObjectExt};
use crate::core::router::GObjAncestry;
use crate::engine::WindowSizePx;
use crate::engine::input::InputTracker;
use crate::engine::gfx::{
    GfxSingletons, Viewport,
    WindowManager, RegisteredWindow,
    ViewportHandler, VIEWPORT_HANDLER_KEY,
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
            app.get(InputTracker::KEY).handle(&event);

            // Handle windowing
            let wm = app.get(WindowManager::KEY);
            wm.handle_event(&ancestry, &event);

            if wm.viewport_map().borrow().is_empty() {
                *flow = ControlFlow::Exit;
                return;
            }
        });
    }
}

impl GameObject for OApplication {
    fn get_raw<'val>(&'val self, out: &mut KeyOut<'_, 'val>) -> bool {
        out.try_put_field(GfxSingletons::KEY, &self.gfx) ||
            out.try_put_field(WindowManager::KEY, &self.wm) ||
            out.try_put_field(InputTracker::KEY, &self.inp)
    }
}

struct MyViewportHandler;

impl ViewportHandler for MyViewportHandler {
    fn window_event(&self, ancestry: &GObjAncestry, window: &Rc<RegisteredWindow>, event: &WindowEvent) {
        if let WindowEvent::CloseRequested = event {
            ancestry.get_obj(WindowManager::KEY).remove(window);
            return;
        }

        let input_tracker = ancestry.get_obj(InputTracker::KEY);
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

impl GameObject for MyViewportHandler {
    fn get_raw<'val>(&'val self, out: &mut KeyOut<'_, 'val>) -> bool {
        out.try_put_field(VIEWPORT_HANDLER_KEY, self)
    }
}
