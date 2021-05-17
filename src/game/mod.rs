use std::rc::Rc;
use winit::dpi::{LogicalSize, PhysicalSize};
use winit::event::WindowEvent;
use winit::event_loop::{EventLoop, ControlFlow};
use winit::window::{WindowBuilder, WindowId, Window};
use crate::core::game_object::{GameObject, KeyOut, GameObjectExt};
use crate::core::router::GObjAncestry;
use crate::engine::gfx::{GfxSingletons, WindowManager, Viewport, ViewportHandler, VIEWPORT_HANDLER_KEY};

pub struct OApplication {
    gfx: GfxSingletons,
    wm: WindowManager,
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
        wm.register(Rc::new(OViewport::new(&gfx, WindowBuilder::new()
            .with_title("Main window")
            .with_inner_size(LogicalSize::new(1920, 1080))
            .build(&event_loop).unwrap())));

        wm.register(Rc::new(OViewport::new(&gfx, WindowBuilder::new()
            .with_title("Other window")
            .with_inner_size(LogicalSize::new(400, 400))
            .build(&event_loop).unwrap())));

        // Start app
        let app = OApplication { gfx, wm };

        event_loop.run(move |event, _proxy, flow| {
            *flow = ControlFlow::Poll;

            let ancestry = GObjAncestry::root(&app);

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
            out.try_put_field(WindowManager::KEY, &self.wm)
    }
}

pub struct OViewport {
    viewport: Viewport,
    handler: MyViewportHandler,
}

impl OViewport {
    pub fn new(gfx: &GfxSingletons, window: Window) -> Self {
        Self {
            viewport: Viewport::new(&gfx, window),
            handler: MyViewportHandler,
        }
    }
}

impl GameObject for OViewport {
    fn get_raw<'val>(&'val self, out: &mut KeyOut<'_, 'val>) -> bool {
        out.try_put_field(Viewport::KEY, &self.viewport) ||
            out.try_put_field(VIEWPORT_HANDLER_KEY, &self.handler)
    }
}

struct MyViewportHandler;

impl ViewportHandler for MyViewportHandler {
    fn window_event(&self, ancestry: &GObjAncestry, win_id: WindowId, event: &WindowEvent) {
        if let WindowEvent::CloseRequested = event {
            ancestry.get_obj(WindowManager::KEY)
                .unregister_by_id(win_id);
        }
    }

    fn resized(&self, _ancestry: &GObjAncestry, _new_size: PhysicalSize<u32>) {
        println!("Resized!");
    }

    fn redraw(&self, _ancestry: &GObjAncestry, _frame: wgpu::SwapChainFrame) {}
}
