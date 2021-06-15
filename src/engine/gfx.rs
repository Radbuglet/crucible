use std::cell::{RefCell, Cell};
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;
use winit::event::WindowEvent;
use winit::window::{Window, WindowId};
use crate::core::game_object::{new_key, Key, GameObject, GameObjectExt};
use crate::core::router::GObjAncestry;
use crate::core::mutability::CellExt;
use crate::engine::{WinitEvent, WindowSizePx};

// === Core GFX === //

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum GfxLoadError {
    NoAdapter,
    NoDevice,
}

impl fmt::Display for GfxLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GfxLoadError::NoAdapter => f.write_str("No suitable adapters could be found."),
            GfxLoadError::NoDevice => f.write_str("No suitable devices could be found.")
        }
    }
}

pub struct GfxSingletons {
    pub instance: wgpu::Instance,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl GfxSingletons {
    pub const KEY: Key<Self> = new_key!(Self);

    async fn request_adapter(instance: &wgpu::Instance, compatible_surface: Option<&wgpu::Surface>) -> Result<wgpu::Adapter, GfxLoadError> {
        instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface
        }).await.ok_or(GfxLoadError::NoAdapter)
    }

    async fn request_device(adapter: &wgpu::Adapter) -> Result<(wgpu::Device, wgpu::Queue), GfxLoadError> {
        adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("main device"),
            features: wgpu::Features::default(),
            limits: wgpu::Limits::default(),
        }, None).await.or(Err(GfxLoadError::NoDevice))
    }

    pub async fn new(backend: wgpu::BackendBit) -> Result<Self, GfxLoadError> {
        let instance = wgpu::Instance::new(backend);
        let adapter = Self::request_adapter(&instance, None).await?;
        let (device, queue) = Self::request_device(&adapter).await?;

        Ok(Self { instance, device, queue })
    }

    pub async fn new_with_window(backend: wgpu::BackendBit, window: Window) -> Result<(Self, Viewport), GfxLoadError> {
        let instance = wgpu::Instance::new(backend);
        let surface = unsafe { instance.create_surface(&window) };
        let adapter = Self::request_adapter(&instance, Some(&surface)).await?;
        let (device, queue) = Self::request_device(&adapter).await?;

        let gfx = Self { instance, device, queue };
        let viewport = Viewport::from_surface(&gfx, window, surface);

        Ok((gfx, viewport))
    }
}

pub struct Viewport {
    window: Window,
    surface: wgpu::Surface,
    inner: RefCell<ViewportInner>,
}

struct ViewportInner {
    swapchain: wgpu::SwapChain,
    swapchain_desc: wgpu::SwapChainDescriptor,
    dirty: bool,
}

impl Viewport {
    // === Constructors === //

    pub fn new(gfx: &GfxSingletons, window: Window) -> Self {
        let surface = unsafe { gfx.instance.create_surface(&window) };
        Self::from_surface(gfx, window, surface)
    }

    pub fn from_surface(gfx: &GfxSingletons, window: Window, surface: wgpu::Surface) -> Self {
        let swapchain_desc = wgpu::SwapChainDescriptor {
            usage: wgpu::TextureUsage::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            width: window.inner_size().width,
            height: window.inner_size().height,
            present_mode: wgpu::PresentMode::Fifo,
        };
        let swapchain = gfx.device.create_swap_chain(&surface, &swapchain_desc);

        Self {
            window,
            surface,
            inner: RefCell::new(ViewportInner {
                swapchain,
                swapchain_desc,
                dirty: false,
            })
        }
    }

    // === Accessors === //

    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn window_id(&self) -> WindowId {
        self.window().id()
    }

    pub fn usage(&self) -> wgpu::TextureUsage {
        self.inner.borrow()
            .swapchain_desc.usage
    }

    pub fn set_usage(&self, usage: wgpu::TextureUsage) {
        let mut inner = self.inner.borrow_mut();
        inner.swapchain_desc.usage = usage;
        inner.dirty = true;
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.inner.borrow()
            .swapchain_desc.format
    }

    pub fn set_format(&self, format: wgpu::TextureFormat) {
        let mut inner = self.inner.borrow_mut();
        inner.swapchain_desc.format = format;
        inner.dirty = true;
    }

    pub fn present_mode(&self) -> wgpu::PresentMode {
        self.inner.borrow()
            .swapchain_desc.present_mode
    }

    pub fn set_present_mode(&self, present_mode: wgpu::PresentMode) {
        let mut inner = self.inner.borrow_mut();
        inner.swapchain_desc.present_mode = present_mode;
        inner.dirty = true;
    }

    // === Rendering functions === //

    pub fn pre_render(&self, gfx: &GfxSingletons) -> PreRenderOp {
        let mut inner = self.inner.borrow_mut();
        let win_sz = self.window.inner_size();

        let mut resized = false;

        if inner.swapchain_desc.width != win_sz.width {
            inner.swapchain_desc.width = win_sz.width;
            resized = true;
        }

        if inner.swapchain_desc.height != win_sz.height {
            inner.swapchain_desc.height = win_sz.height;
            inner.dirty = true;
        }

        if inner.dirty || resized {
            inner.swapchain = gfx.device.create_swap_chain(&self.surface, &inner.swapchain_desc);
            inner.dirty = false;
        }

        if resized {
            PreRenderOp::Resized(win_sz)
        } else {
            PreRenderOp::None
        }
    }

    pub fn get_frame(&self) -> Result<wgpu::SwapChainFrame, wgpu::SwapChainError> {
        self.inner.borrow()
            .swapchain
            .get_current_frame()
    }
}

#[derive(Debug, Copy, Clone)]
pub enum PreRenderOp {
    Resized(WindowSizePx),
    None,
}


// === Windowing integration === //

#[derive(Default)]
pub struct WindowManager {
    windows: RefCell<HashMap<WindowId, Rc<RegisteredWindow>>>,
}

impl WindowManager {
    pub const KEY: Key<Self> = new_key!(Self);

    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&self, viewport: Viewport, handler: Rc<dyn GameObject>) -> WindowId {
        debug_assert!(handler.has_key(VIEWPORT_HANDLER_KEY));

        let window_id = viewport.window_id();
        let window = Rc::new(RegisteredWindow {
            viewport,
            handler: Cell::new(handler),
        });

        self.windows.borrow_mut().insert(window_id, window);
        window_id
    }

    pub fn remove(&self, win: &Rc<RegisteredWindow>) {
        self.remove_by_id(&win.viewport().window_id());
    }

    pub fn remove_by_id(&self, id: &WindowId) {
        self.windows.borrow_mut().remove(id);
    }

    pub fn get_window(&self, id: &WindowId) -> Option<Rc<RegisteredWindow>> {
        self.windows.borrow()
            .get(id)
            .map(Rc::clone)
    }

    pub fn viewport_map(&self) -> &RefCell<HashMap<WindowId, Rc<RegisteredWindow>>> {
        &self.windows
    }

    pub fn handle_event(&self, ancestry: &GObjAncestry, event: &WinitEvent) {
        match event {
            WinitEvent::RedrawRequested(win_id) => {
                if let Some(window) = self.get_window(win_id) {
                    let handler_obj = window.handler();
                    let ancestry = ancestry.child(&*handler_obj);
                    let handler = handler_obj.fetch_key(VIEWPORT_HANDLER_KEY);

                    // Pre-render
                    if let PreRenderOp::Resized(size) = window
                        .viewport()
                        .pre_render(ancestry.get_obj(GfxSingletons::KEY))
                    {
                        handler.resized(&ancestry, &window, size);
                    }

                    // Dispatch redraw
                    if let Ok(frame) = window.viewport().get_frame() {
                        handler.redraw(&ancestry, &window, frame);
                    } else {
                        eprintln!("Failed to get swapchain frame!");
                    }
                }
            }
            WinitEvent::WindowEvent { event, window_id: win_id } => {
                if let Some(window) = self.get_window(win_id) {
                    let handler_obj = window.handler();
                    let ancestry = ancestry.child(&*handler_obj);
                    handler_obj
                        .fetch_key(VIEWPORT_HANDLER_KEY)
                        .window_event(&ancestry, &window, event);
                }
            }
            _ => {}
        }
    }
}

pub struct RegisteredWindow {
    viewport: Viewport,
    handler: Cell<Rc<dyn GameObject>>,
}

impl RegisteredWindow {
    pub fn set_handler(&self, handler: Rc<dyn GameObject>) {
        self.handler.set(handler);
    }

    pub fn handler(&self) -> Rc<dyn GameObject> {
        self.handler.clone_inner()
    }

    pub fn viewport(&self) -> &Viewport {
        &self.viewport
    }
}

pub const VIEWPORT_HANDLER_KEY: Key<dyn ViewportHandler> = new_key!(dyn ViewportHandler);

pub trait ViewportHandler {
    fn window_event(&self, ancestry: &GObjAncestry, window: &Rc<RegisteredWindow>, event: &WindowEvent);
    fn resized(&self, ancestry: &GObjAncestry, window: &Rc<RegisteredWindow>, new_size: WindowSizePx);
    fn redraw(&self, ancestry: &GObjAncestry, window: &Rc<RegisteredWindow>, frame: wgpu::SwapChainFrame);
}
