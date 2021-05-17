use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::window::{Window, WindowId};
use crate::core::game_object::{new_key, Key, GameObject, GameObjectExt};
use crate::core::router::GObjAncestry;
use super::WinitEvent;

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

#[derive(Default)]
pub struct WindowManager {
    viewports: RefCell<HashMap<WindowId, Rc<dyn GameObject>>>,
}

impl WindowManager {
    pub const KEY: Key<Self> = new_key!(Self);

    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, viewport: Rc<dyn GameObject>) {
        debug_assert!(viewport.has(VIEWPORT_HANDLER_KEY), "Viewport must have an attached handler!");
        debug_assert!(viewport.has(Viewport::KEY), "Viewport must have an attached `Viewport` instance!");

        let window_id = viewport.get(Viewport::KEY).window_id();
        self.viewports.borrow_mut()
            .insert(window_id, viewport);
    }

    pub fn unregister(&self, viewport: &dyn GameObject) {
        self.unregister_by_id(viewport.get(Viewport::KEY).window_id());
    }

    pub fn unregister_by_id(&self, id: WindowId) {
        self.viewports.borrow_mut()
            .remove(&id);
    }

    pub fn fetch_viewport(&self, id: WindowId) -> Option<Rc<dyn GameObject>> {
        self.viewports.borrow()
            .get(&id)
            .map(Rc::clone)
    }

    pub fn viewport_map(&self) -> &RefCell<HashMap<WindowId, Rc<dyn GameObject>>> {
        &self.viewports
    }

    pub fn handle_event(&self, ancestry: &GObjAncestry, event: &WinitEvent) {
        let gfx = ancestry.get_obj(GfxSingletons::KEY);

        match event {
            WinitEvent::RedrawRequested(window_id) => {
                let viewport_obj = self.fetch_viewport(*window_id);

                if let Some(viewport_obj) = viewport_obj {
                    let ancestry = ancestry.child(&*viewport_obj);
                    let viewport = viewport_obj.get(Viewport::KEY);
                    let handler = viewport_obj.get(VIEWPORT_HANDLER_KEY);

                    if viewport.pre_render(gfx) {
                        handler.resized(&ancestry, viewport.window().inner_size());
                    }

                    match viewport.get_frame() {
                        Ok(frame) => handler.redraw(&ancestry, frame),
                        Err(err) => eprintln!("Error while polling swapchain: \"{}\"", err)
                    }
                }
            }
            WinitEvent::WindowEvent { window_id, event } => {
                let viewport_obj = self.fetch_viewport(*window_id);

                if let Some(viewport_obj) = viewport_obj {
                    let ancestry = ancestry.child(&*viewport_obj);
                    viewport_obj.get(VIEWPORT_HANDLER_KEY)
                        .window_event(&ancestry, *window_id, event);
                }
            }
            _ => {}
        }
    }
}

pub const VIEWPORT_HANDLER_KEY: Key<dyn ViewportHandler> = new_key!(dyn ViewportHandler);

pub trait ViewportHandler {
    fn window_event(&self, ancestry: &GObjAncestry, win_id: WindowId, event: &WindowEvent);
    fn resized(&self, ancestry: &GObjAncestry, new_size: PhysicalSize<u32>);
    fn redraw(&self, ancestry: &GObjAncestry, frame: wgpu::SwapChainFrame);
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
    pub const KEY: Key<Self> = new_key!(Self);

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

    pub fn pre_render(&self, gfx: &GfxSingletons) -> bool {
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

        resized
    }

    pub fn get_frame(&self) -> Result<wgpu::SwapChainFrame, wgpu::SwapChainError> {
        self.inner.borrow()
            .swapchain
            .get_current_frame()
    }
}
