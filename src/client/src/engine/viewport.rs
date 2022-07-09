use crate::engine::gfx::GfxContext;
use crucible_common::util::cell::filter_map_ref;
use geode::prelude::*;
use std::{cell::Ref, collections::HashMap};
use thiserror::Error;
use winit::window::{Window, WindowId};

pub const THE_ONE_SURFACE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;

// TODO: Dynamically adapt format; figure out color spaces
// fn get_preferred_format(surface: &wgpu::Surface, adapter: &wgpu::Adapter) -> wgpu::TextureFormat {
// 	surface
// 		.get_supported_formats(adapter)
// 		.get(0)
// 		.copied()
// 		.expect("Surface is incompatible with adapter.")
// }

#[derive(Debug, Default)]
pub struct ViewportManager {
	viewports: HashMap<WindowId, Owned<Entity>>,
}

impl ViewportManager {
	pub fn register(
		&mut self,
		s: Session,
		main_lock: Lock,
		gfx: &GfxContext,
		target: Owned<Entity>,
		window: Window,
		surface: wgpu::Surface,
	) {
		let win_id = window.id();
		let win_size = window.inner_size();
		let config = wgpu::SurfaceConfiguration {
			usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
			format: THE_ONE_SURFACE_FORMAT,
			width: win_size.width,
			height: win_size.height,
			present_mode: wgpu::PresentMode::Fifo,
		};
		surface.configure(&gfx.device, &config);
		target.add(
			s,
			Viewport {
				window: Some(window),
				surface,
				config,
				config_changed: false,
			}
			.box_obj_rw(s, main_lock),
		);
		self.viewports.insert(win_id, target);
	}

	pub fn unregister(&mut self, id: WindowId) -> Option<Owned<Entity>> {
		self.viewports.remove(&id)
	}

	pub fn get_viewport(&self, id: WindowId) -> Option<Entity> {
		self.viewports.get(&id).map(|viewport| **viewport)
	}

	pub fn all_viewports(&self) -> impl Iterator<Item = (WindowId, Entity)> + '_ {
		self.viewports.iter().map(|(k, v)| (*k, **v))
	}

	pub fn mounted_viewports<'a>(
		&'a self,
		s: Session<'a>,
	) -> impl Iterator<Item = (WindowId, Entity, Ref<'a, Window>)> + 'a {
		self.all_viewports().filter_map(move |(window_id, entity)| {
			let viewport = entity.borrow::<Viewport>(s);
			let window = filter_map_ref(viewport, |viewport| viewport.window()).ok()?;

			Some((window_id, entity, window))
		})
	}
}

#[derive(Debug)]
pub struct Viewport {
	window: Option<Window>,
	surface: wgpu::Surface,
	config: wgpu::SurfaceConfiguration,
	config_changed: bool,
}

impl Viewport {
	pub fn render(
		&mut self,
		gfx: &GfxContext,
	) -> Result<Option<wgpu::SurfaceTexture>, OutOfDeviceMemoryError> {
		use wgpu::SurfaceError::*;

		let window = self
			.window
			.as_ref()
			.expect("attempted to render to unmounted viewport");

		// Ensure that the surface texture matches the window's physical (backing buffer) size
		let win_size = window.inner_size();

		let preferred_format = THE_ONE_SURFACE_FORMAT;

		if self.config.format != preferred_format {
			self.config.format = preferred_format;
			self.config_changed = true;
		}

		if self.config.width != win_size.width {
			self.config.width = win_size.width;
			self.config_changed = true;
		}

		if self.config.height != win_size.height {
			self.config.height = win_size.height;
			self.config_changed = true;
		}

		if self.config_changed {
			self.surface.configure(&gfx.device, &self.config);
			self.config_changed = false;
		}

		match self.surface.get_current_texture() {
			Ok(frame) => Ok(Some(frame)),
			Err(Timeout) => {
				log::warn!(
					"Request to acquire swap-chain for window {:?} timed out.",
					window.id()
				);
				Ok(None)
			}
			Err(OutOfMemory) => Err(OutOfDeviceMemoryError),
			Err(Outdated) | Err(Lost) => {
				log::warn!(
					"Swap-chain for window {:?} is outdated or was lost.",
					window.id()
				);

				// Try to recreate the swap-chain and try again.
				self.surface.configure(&gfx.device, &self.config);

				match self.surface.get_current_texture() {
					Ok(frame) => Ok(Some(frame)),
					Err(OutOfMemory) => Err(OutOfDeviceMemoryError),
					_ => {
						log::warn!(
							"Failed to acquire swap-chain for window {:?} after swap-chain was recreated.",
							window.id()
						);
						Ok(None)
					}
				}
			}
		}
	}

	pub fn window(&self) -> Option<&Window> {
		self.window.as_ref()
	}

	pub fn unmount(&mut self) -> Option<Window> {
		self.window.take()
	}
}

#[derive(Debug, Copy, Clone, Error)]
#[error("out of device memory")]
pub struct OutOfDeviceMemoryError;

event_trait! {
	pub trait ViewportRenderHandler::on_render(
		&self,
		frame: Option<wgpu::SurfaceTexture>,
		s: Session,
		me: Entity,
		viewport: Entity,
		engine: Entity,
	);
}
