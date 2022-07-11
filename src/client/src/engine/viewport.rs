use crate::engine::gfx::GfxContext;
use crucible_core::cell::filter_map_ref;
use geode::prelude::*;
use std::{cell::Ref, collections::HashMap};
use thiserror::Error;
use typed_glam::glam::UVec2;
use winit::window::{Window, WindowId};

pub const FALLBACK_SURFACE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;

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
			format: FALLBACK_SURFACE_FORMAT,
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

fn surface_size_from_config(config: &wgpu::SurfaceConfiguration) -> Option<UVec2> {
	let size = UVec2::new(config.width, config.height);

	// We also don't really want 1x1 surfaces in case we ever want to subtract one from the
	// dimension.
	if size.x < 2 || size.y < 2 {
		None
	} else {
		Some(size)
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
	pub fn set_format(&mut self, format: wgpu::TextureFormat) {
		if self.config.format != format {
			self.config.format = format;
			self.config_changed = true;
		}
	}

	pub fn format(&self) -> wgpu::TextureFormat {
		self.config.format
	}

	pub fn set_present_mode(&mut self, mode: wgpu::PresentMode) {
		if self.config.present_mode != mode {
			self.config.present_mode = mode;
			self.config_changed = true;
		}
	}

	pub fn present_mode(&self) -> wgpu::PresentMode {
		self.config.present_mode
	}

	pub fn surface_size(&self) -> Option<UVec2> {
		surface_size_from_config(&self.config)
	}

	pub fn surface_aspect(&self) -> Option<f32> {
		self.surface_size().map(|size| {
			let size = size.as_vec2();
			size.x / size.y
		})
	}

	pub fn render(
		&mut self,
		gfx: &GfxContext,
	) -> Result<Option<wgpu::SurfaceTexture>, OutOfDeviceMemoryError> {
		use wgpu::SurfaceError::*;

		fn normalize_swapchain_config(
			surface: &wgpu::Surface,
			config: &mut wgpu::SurfaceConfiguration,
			config_changed: &mut bool,
			gfx: &GfxContext,
			window: &Window,
		) -> bool {
			// Ensure that we're still using a supported format.
			let supported_formats = surface.get_supported_formats(&gfx.adapter);

			assert!(
				supported_formats.len() > 0,
				"The current graphics adapter does not support this surface."
			);

			if supported_formats.contains(&config.format) {
				log::warn!(
					"Swapchain format {:?} is unsupported by surface-adapter pair. Falling back to {:?}.",
					config.format,
					FALLBACK_SURFACE_FORMAT
				);
				config.format = FALLBACK_SURFACE_FORMAT;
				*config_changed = true;
			}

			// Ensure that the surface texture matches the window's physical (backing buffer) size
			let win_size = window.inner_size();

			if config.width != win_size.width {
				config.width = win_size.width;
				*config_changed = true;
			}

			if config.height != win_size.height {
				config.height = win_size.height;
				*config_changed = true;
			}

			// Ensure that we can actually render to the surface
			if surface_size_from_config(config).is_none() {
				return false;
			}

			true
		}

		// Get window
		let window = self
			.window
			.as_ref()
			.expect("attempted to render to unmounted viewport");

		// Normalize the swapchain
		if !normalize_swapchain_config(
			&self.surface,
			&mut self.config,
			&mut self.config_changed,
			gfx,
			window,
		) {
			return Ok(None);
		}

		// Try to reconfigure the surface if it was updated
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

				// Renormalize the swapchain config
				// This is done in case the swapchain settings changed since then. This event is
				// exceedingly rare but we're already in the slow path anyways so we might as well
				// do things right.
				if !normalize_swapchain_config(
					&self.surface,
					&mut self.config,
					&mut self.config_changed,
					gfx,
					window,
				) {
					return Ok(None);
				}

				// Try to recreate the swapchain and try again
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
