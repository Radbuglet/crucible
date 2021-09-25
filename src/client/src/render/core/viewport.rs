use crate::render::core::context::GfxContext;
use crate::util::vec_ext::VecConvert;
use cgmath::{Vector2, Zero};
use core::foundation::prelude::*;
use std::collections::HashMap;
use winit::window::{Window, WindowId};

#[derive(Default)]
pub struct ViewportManager {
	windows: HashMap<WindowId, Entity>,
	viewports: MapStorage<Viewport>,
}

impl ViewportManager {
	pub fn new() -> Self {
		Default::default()
	}

	pub fn register(&mut self, gfx: &GfxContext, entity: Entity, window: Window) {
		let surface = unsafe { gfx.instance.create_surface(&window) };
		self.register_pair(gfx, entity, window, surface);
	}

	/// Registers a viewport for the `window` and its `surface` and attaches it to the provided
	/// `entity`.
	pub fn register_pair(
		&mut self,
		gfx: &GfxContext,
		entity: Entity,
		window: Window,
		surface: wgpu::Surface,
	) {
		let win_id = window.id();

		let old_viewport = self
			.viewports
			.insert(entity, Viewport::new(gfx, window, surface));
		let old_window = self.windows.insert(win_id, entity);

		debug_assert!(
			old_viewport.is_none(),
			"Cannot assign multiple viewports to the same entity."
		);

		debug_assert!(
			old_window.is_none(),
			"Cannot assign a window to multiple viewports."
		);
	}

	/// Maps `WindowId` to `Entity`.
	pub fn get_entity(&self, id: WindowId) -> Option<Entity> {
		self.windows.get(&id).copied()
	}

	/// Gets all registered viewport entities.
	pub fn get_entities(&self) -> impl ExactSizeIterator + Iterator<Item = Entity> + '_ {
		self.windows.values().copied()
	}

	/// Gets a viewport for a given `Entity`.
	pub fn get_viewport(&self, id: Entity) -> Option<&Viewport> {
		self.viewports.get(id)
	}

	/// Gets a viewport for a given `Entity`.
	pub fn get_viewport_mut(&mut self, id: Entity) -> Option<&mut Viewport> {
		self.viewports.get_mut(id)
	}

	/// Unregisters a viewport with a given `WindowId`.
	pub fn unregister(&mut self, id: WindowId) {
		let entity_id = self.windows.remove(&id).unwrap();
		self.viewports.remove(entity_id);
	}
}

pub struct Viewport {
	window: Window,
	surface: wgpu::Surface,
	config: wgpu::SurfaceConfiguration,
}

impl Viewport {
	pub fn new(gfx: &GfxContext, window: Window, surface: wgpu::Surface) -> Self {
		let config = wgpu::SurfaceConfiguration {
			present_mode: wgpu::PresentMode::Fifo,
			format: wgpu::TextureFormat::Bgra8UnormSrgb,
			width: window.inner_size().width,
			height: window.inner_size().height,
			usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
		};
		surface.configure(&gfx.device, &config);
		Self {
			window,
			surface,
			config,
		}
	}

	pub fn redraw(&mut self, gfx: &GfxContext) -> Option<wgpu::SurfaceTexture> {
		// Attempt to get a new frame from the current swapchain
		if self.window.inner_size().to_vec() == Vector2::new(self.config.width, self.config.height)
		{
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
		}

		// Re-create and try again
		log::trace!("Recreating surface {:?}", self.surface);
		let size = self.window.inner_size().to_vec();
		if size.is_zero() {
			return None;
		}
		self.config.width = size.x;
		self.config.height = size.y;
		self.surface.configure(&gfx.device, &self.config);

		// Second attempt
		self.surface
			.get_current_frame()
			.ok()
			.map(|frame| frame.output)
	}

	pub fn id(&self) -> WindowId {
		self.window.id()
	}

	pub fn window(&self) -> &Window {
		&self.window
	}
}
