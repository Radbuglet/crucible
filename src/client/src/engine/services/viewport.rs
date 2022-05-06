use crate::engine::services::gfx::GfxContext;
use geode::prelude::*;
use std::collections::HashMap;
use thiserror::Error;
use winit::window::{Window, WindowId};

const DEFAULT_SURFACE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;

#[derive(Debug, Default)]
pub struct ViewportManager {
	viewports: HashMap<WindowId, Obj>,
}

impl ViewportManager {
	pub fn register(
		&mut self,
		gfx: &GfxContext,
		mut target: Obj,
		window: Window,
		surface: wgpu::Surface,
	) {
		let win_id = window.id();
		let win_size = window.inner_size();
		let config = wgpu::SurfaceConfiguration {
			usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
			format: surface
				.get_preferred_format(&gfx.adapter)
				.unwrap_or(DEFAULT_SURFACE_FORMAT),
			width: win_size.width,
			height: win_size.height,
			present_mode: wgpu::PresentMode::Fifo,
		};
		surface.configure(&gfx.device, &config);
		target.add_rw(Viewport {
			window,
			surface,
			config,
			config_changed: false,
		});
		self.viewports.insert(win_id, target);
	}

	pub fn unregister(&mut self, id: WindowId) -> Option<Obj> {
		self.viewports.remove(&id)
	}

	pub fn get_viewport(&self, id: WindowId) -> Option<&Obj> {
		self.viewports.get(&id)
	}

	pub fn viewports(&self) -> impl Iterator<Item = (WindowId, &Obj)> {
		self.viewports.iter().map(|(k, v)| (*k, v))
	}
}

#[derive(Debug)]
pub struct Viewport {
	window: Window,
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

		// Ensure that the surface texture matches the window's physical (backing buffer) size
		let win_size = self.window.inner_size();

		let preferred_format = self
			.surface
			.get_preferred_format(&gfx.adapter)
			.unwrap_or(DEFAULT_SURFACE_FORMAT);

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
					self.window.id()
				);
				Ok(None)
			}
			Err(OutOfMemory) => Err(OutOfDeviceMemoryError),
			Err(Outdated) | Err(Lost) => {
				log::warn!(
					"Swap-chain for window {:?} is outdated or was lost.",
					self.window.id()
				);

				// Try to recreate the swap-chain and try again.
				self.surface.configure(&gfx.device, &self.config);

				match self.surface.get_current_texture() {
					Ok(frame) => Ok(Some(frame)),
					Err(OutOfMemory) => Err(OutOfDeviceMemoryError),
					_ => {
						log::warn!(
							"Failed to acquire swap-chain for window {:?} after swap-chain was recreated.",
							self.window.id()
						);
						Ok(None)
					}
				}
			}
		}
	}

	pub fn window(&self) -> &Window {
		&self.window
	}
}

#[derive(Debug, Copy, Clone, Error)]
#[error("out of device memory")]
pub struct OutOfDeviceMemoryError;
