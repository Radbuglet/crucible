use std::cell::RefCell;
use std::rc::Rc;
use winit::dpi::{LogicalSize, PhysicalSize};
use winit::event::{Event as WinitEvent, WindowEvent};
use winit::event_loop::{EventLoop, ControlFlow};
use winit::window::WindowBuilder;
use crate::core::game_object::{GameObject, KeyOut, GameObjectExt};
use crate::core::router::GObjAncestry;
use crate::engine::gfx::{GfxSingletons, WindowManager, Viewport, ViewportHandler, VIEWPORT_HANDLER_KEY};

pub struct OApplication {
    gfx: GfxSingletons,
    wm: RefCell<WindowManager>,
}

impl OApplication {
    pub fn start() -> ! {
        // Set up GraphicsSingletons
        let event_loop = EventLoop::new();
        let (gfx, main_viewport) = futures::executor::block_on({
            let main_window = WindowBuilder::new()
                .with_title("Crucible")
                .with_inner_size(LogicalSize::new(1920, 1080))
                .with_min_inner_size(PhysicalSize::new(100, 100))
                .with_visible(false)
                .build(&event_loop).unwrap();
            GfxSingletons::new(wgpu::BackendBit::PRIMARY, main_window)
        }).unwrap();

        // Construct windows
        let mut wm = WindowManager::new();
        let main_viewport = Rc::new(OViewport {
            viewport: main_viewport,
            handler: MyViewportHandler,
        });
        main_viewport.viewport.window().set_visible(true);
        wm.register(main_viewport);

        // Start app
        let app = OApplication {
            gfx,
            wm: RefCell::new(wm),
        };

        event_loop.run(move |event, _proxy, flow| {
            *flow = ControlFlow::Poll;

            if let WinitEvent::WindowEvent { event: WindowEvent::CloseRequested, .. } = &event {
                *flow = ControlFlow::Exit;
                return;
            }

            let ancestry = GObjAncestry::root(&app);
            app.get(WindowManager::KEY)
                .borrow()
                .handle_event(&ancestry, &event);
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

impl GameObject for OViewport {
    fn get_raw<'val>(&'val self, out: &mut KeyOut<'_, 'val>) -> bool {
        out.try_put_field(Viewport::KEY, &self.viewport) ||
            out.try_put_field(VIEWPORT_HANDLER_KEY, &self.handler)
    }
}

struct MyViewportHandler;

impl ViewportHandler for MyViewportHandler {
    fn window_event(&self, _ancestry: &GObjAncestry, _event: &WindowEvent) {}

    fn resized(&self, _ancestry: &GObjAncestry, _new_size: PhysicalSize<u32>) {
        println!("Resized!");
    }

    fn redraw(&self, _ancestry: &GObjAncestry, _frame: wgpu::SwapChainFrame) {}
}
