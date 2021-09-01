use crate::render::core::context::GfxContext;
use crate::util::winit::WinitEventBundle;
use std::collections::HashMap;
use winit::event::{Event, WindowEvent};
use winit::window::{Window, WindowId};

pub struct GfxManager {
	cx: GfxContext,
	viewports: HashMap<WindowId, Viewport>,
}

impl GfxManager {
	pub fn new_with_window(window: Window) -> anyhow::Result<(Self, WindowId)> {
		Self::new(Some(window)).map(|(mg, win_id)| (mg, win_id.unwrap()))
	}

	pub fn new(window: Option<Window>) -> anyhow::Result<(Self, Option<WindowId>)> {
		let (cx, surface) = GfxContext::new(window.as_ref())?;
		let mut mg = Self {
			cx,
			viewports: Default::default(),
		};

		let window_id = if let Some(window) = window {
			let viewport = Viewport::new(&mg.cx, window, surface.unwrap());
			let id = viewport.id();
			mg.viewports.insert(id, viewport);
			Some(id)
		} else {
			None
		};

		Ok((mg, window_id))
	}

	pub fn make_viewport(&mut self, window: Window) -> WindowId {
		let surface = unsafe { self.cx.instance.create_surface(&window) };
		let viewport = Viewport::new(&self.cx, window, surface);

		let id = viewport.id();
		self.viewports.insert(id, viewport);
		id
	}

	pub fn get_window(&self, id: &WindowId) -> &Window {
		self.viewports
			.get(id)
			.map(|viewport| &viewport.window)
			.unwrap()
	}

	pub fn handle_ev(&mut self, (ev, _, _): WinitEventBundle) {
		match ev {
			Event::WindowEvent {
				event: WindowEvent::CloseRequested,
				window_id,
			} => {
				if let Some(viewport) = self.viewports.remove(window_id) {
					println!("Closed window {:?}", viewport.id());
				}
			}
			Event::RedrawRequested(window_id) => {
				if let Some(viewport) = self.viewports.get_mut(window_id) {
					if let Some(_) = viewport.redraw(&self.cx) {}
				}
			}
			Event::MainEventsCleared => {
				for viewport in self.viewports.values() {
					viewport.window.request_redraw();
				}
			}
			_ => {}
		}
	}
}

struct Viewport {
	window: Window,
	surface: wgpu::Surface,
	config: wgpu::SurfaceConfiguration,
}

impl Viewport {
	fn new(cx: &GfxContext, window: Window, surface: wgpu::Surface) -> Self {
		let config = wgpu::SurfaceConfiguration {
			present_mode: wgpu::PresentMode::Fifo,
			format: wgpu::TextureFormat::Bgra8UnormSrgb,
			width: window.inner_size().width,
			height: window.inner_size().height,
			usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
		};
		surface.configure(&cx.device, &config);
		Self {
			window,
			surface,
			config,
		}
	}

	fn redraw(&mut self, cx: &GfxContext) -> Option<wgpu::SurfaceTexture> {
		// First attempt
		match self.surface.get_current_frame() {
			// Desired outcome
			Ok(wgpu::SurfaceFrame {
				suboptimal: false,
				output,
			}) => return Some(output),

			// Unrecoverable
			Err(wgpu::SurfaceError::Timeout) => return None,
			Err(wgpu::SurfaceError::OutOfMemory) => return None,

			// Try re-create
			Ok(wgpu::SurfaceFrame {
				suboptimal: true, ..
			}) => (),
			Err(wgpu::SurfaceError::Outdated) => (),
			Err(wgpu::SurfaceError::Lost) => (),
		}

		// Re-create and try again
		println!("Rebuilt window {:?}", self.window);
		self.config.width = self.window.inner_size().width;
		self.config.height = self.window.inner_size().height;
		self.surface.configure(&cx.device, &self.config);

		// Second attempt
		self.surface
			.get_current_frame()
			.ok()
			.map(|frame| frame.output)
	}

	fn id(&self) -> WindowId {
		self.window.id()
	}
}
