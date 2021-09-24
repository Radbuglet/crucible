use crate::render::core::context::GfxContext;
use crate::util::vec_ext::VecConvert;
use crate::util::winit::{WinitEvent, WinitEventBundle};
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

	pub fn register(&mut self, (gfx,): (&GfxContext,), entity: Entity, window: Window) {
		let surface = unsafe { gfx.instance.create_surface(&window) };
		self.register_pair((gfx,), entity, window, surface);
	}

	pub fn register_pair(
		&mut self,
		(gfx,): (&GfxContext,),
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

	pub fn get_entity(&self, id: WindowId) -> Option<Entity> {
		self.windows.get(&id).copied()
	}

	pub fn get_entities(&self) -> impl Iterator<Item = Entity> + '_ {
		self.windows.values().copied()
	}

	pub fn get_viewport(&self, id: Entity) -> Option<&Viewport> {
		self.viewports.get(id)
	}

	pub fn unregister(&mut self, id: WindowId) {
		let entity_id = self.windows.remove(&id).unwrap();
		self.viewports.remove(entity_id);
	}

	pub fn handle_ev(
		&mut self,
		(gfx,): (&GfxContext,),
		(ev, _, _): WinitEventBundle,
		ev_on_redraw: &mut impl EventPusher<Event = (Entity, wgpu::SurfaceTexture)>,
	) {
		match ev {
			WinitEvent::RedrawRequested(window_id) => {
				if let Some(entity) = self.get_entity(*window_id) {
					let mut viewport = self.viewports.get_mut(entity).unwrap();
					if let Some(texture) = viewport.redraw(gfx) {
						ev_on_redraw.push((entity, texture));
					}
				}
			}
			_ => {}
		}
	}
}

pub struct Viewport {
	window: Window,
	surface: wgpu::Surface,
	config: wgpu::SurfaceConfiguration,
}

impl Viewport {
	fn new(gfx: &GfxContext, window: Window, surface: wgpu::Surface) -> Self {
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

	fn redraw(&mut self, gfx: &GfxContext) -> Option<wgpu::SurfaceTexture> {
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
		println!("Recreating swapchain!");
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

	fn id(&self) -> WindowId {
		self.window.id()
	}

	pub fn window(&self) -> &Window {
		&self.window
	}
}
