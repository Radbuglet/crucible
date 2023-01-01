use crucible_core::{
	ecs::{
		bundle::{bundle, Bundle},
		context::{decompose, unpack},
		entity::Entity,
		event::{EntityDestroyEvent, EventQueueIter},
		storage::Storage,
		universe::{ArchetypeHandle, BuildableArchetypeBundle, ResRw, Universe},
	},
	lang::explicitly_bind::ExplicitlyBind,
};
use hashbrown::HashMap;
use thiserror::Error;
use typed_glam::glam::UVec2;
use winit::window::{Window, WindowId};

use crate::engine::gfx::texture::FullScreenTexture;

use super::{gfx::GfxContext, input::InputManager};

pub const FALLBACK_SURFACE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;

// === ViewportManager === //

#[derive(Debug, Default)]
pub struct ViewportManager {
	window_map: HashMap<WindowId, Entity>,
}

impl ViewportManager {
	pub fn register(&mut self, (viewports,): (&Storage<Viewport>,), viewport: Entity) {
		self.window_map
			.insert(viewports[viewport].window().id(), viewport);
	}

	pub fn get_viewport(&self, id: WindowId) -> Option<Entity> {
		self.window_map.get(&id).copied()
	}

	pub fn window_map(&self) -> &HashMap<WindowId, Entity> {
		&self.window_map
	}

	pub fn unregister(&mut self, window_id: WindowId) {
		self.window_map.remove(&window_id);
	}
}

// === Viewport === //

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
	window: Window,
	surface: ExplicitlyBind<wgpu::Surface>,
	curr_config: wgpu::SurfaceConfiguration,
	next_config: wgpu::SurfaceConfiguration,
	config_dirty: bool,
}

impl Viewport {
	pub fn new(
		(gfx,): (&GfxContext,),
		window: Window,
		surface: Option<wgpu::Surface>,
		config: wgpu::SurfaceConfiguration,
	) -> Self {
		let surface = surface.unwrap_or_else(|| unsafe {
			// Safety: the surface lives for a strictly shorter lifetime than the window
			gfx.instance.create_surface(&window)
		});

		Self {
			window,
			surface: surface.into(),
			curr_config: config.clone(),
			next_config: config,
			config_dirty: false,
		}
	}

	pub fn curr_config(&self) -> &wgpu::SurfaceConfiguration {
		&self.curr_config
	}

	pub fn next_config(&self) -> &wgpu::SurfaceConfiguration {
		&self.next_config
	}

	pub fn set_next_config(&mut self, config: wgpu::SurfaceConfiguration) {
		self.next_config = config;
		self.config_dirty = true;
	}

	pub fn set_usage(&mut self, usage: wgpu::TextureUsages) {
		self.next_config.usage = usage;
		self.config_dirty = true;
	}

	pub fn set_format(&mut self, format: wgpu::TextureFormat) {
		self.next_config.format = format;
		self.config_dirty = true;
	}

	pub fn set_present_mode(&mut self, present_mode: wgpu::PresentMode) {
		self.next_config.present_mode = present_mode;
		self.config_dirty = true;
	}

	pub fn set_alpha_mode(&mut self, alpha_mode: wgpu::CompositeAlphaMode) {
		self.next_config.alpha_mode = alpha_mode;
		self.config_dirty = true;
	}

	pub fn curr_surface_size(&self) -> Option<UVec2> {
		surface_size_from_config(&self.curr_config)
	}

	pub fn curr_surface_aspect(&self) -> Option<f32> {
		self.curr_surface_size().map(|size| {
			let size = size.as_vec2();
			size.x / size.y
		})
	}

	pub fn window(&self) -> &Window {
		&self.window
	}

	pub fn present(
		&mut self,
		(gfx,): (&GfxContext,),
	) -> Result<Option<wgpu::SurfaceTexture>, OutOfDeviceMemoryError> {
		use wgpu::SurfaceError::*;

		fn normalize_swapchain_config(
			gfx: &GfxContext,
			window: &Window,
			surface: &wgpu::Surface,
			config: &mut wgpu::SurfaceConfiguration,
			config_changed: &mut bool,
		) -> bool {
			// Ensure that we're still using a supported format.
			let supported_formats = surface.get_supported_formats(&gfx.adapter);

			assert!(
				!supported_formats.is_empty(),
				"The current graphics adapter does not support this surface."
			);

			if config.format != FALLBACK_SURFACE_FORMAT
				&& !supported_formats.contains(&config.format)
			{
				log::warn!(
					"Swapchain format {:?} is unsupported by surface-adapter pair. Falling back to {:?}.",
					config.format,
					FALLBACK_SURFACE_FORMAT
				);
				config.format = FALLBACK_SURFACE_FORMAT;
				*config_changed = true;
			}

			debug_assert!(supported_formats.contains(&config.format));

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
		// Normalize the swapchain
		if !normalize_swapchain_config(
			gfx,
			&self.window,
			&self.surface,
			&mut self.next_config,
			&mut self.config_dirty,
		) {
			return Ok(None);
		}

		// Try to reconfigure the surface if it was updated
		if self.config_dirty {
			self.surface.configure(&gfx.device, &self.next_config);
			self.curr_config = self.next_config.clone();
			self.config_dirty = false;
		}

		// Acquire the frame
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

				// Renormalize the swapchain config
				// This is done in case the swapchain settings changed since then. This event is
				// exceedingly rare but we're already in the slow path anyways so we might as well
				// do things right.
				if !normalize_swapchain_config(
					gfx,
					&self.window,
					&self.surface,
					&mut self.next_config,
					&mut self.config_dirty,
				) {
					return Ok(None);
				}

				if self.config_dirty {
					self.curr_config = self.next_config.clone();
					self.config_dirty = false;
				}

				// Try to recreate the swapchain and try again
				self.surface.configure(&gfx.device, &self.next_config);

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
}

impl Drop for Viewport {
	fn drop(&mut self) {
		// Ensure that the surface gets dropped before the window
		ExplicitlyBind::drop(&mut self.surface)
	}
}

#[derive(Debug, Copy, Clone, Error)]
#[error("out of device memory")]
pub struct OutOfDeviceMemoryError;

// === ViewportBundle === //

bundle! {
	#[derive(Debug)]
	pub struct ViewportBundle {
		pub viewport: Viewport,
		pub input_manager: InputManager,
		pub depth_texture: FullScreenTexture,
	}
}

impl BuildableArchetypeBundle for ViewportBundle {
	fn create_archetype(universe: &Universe) -> ArchetypeHandle<Self> {
		let arch = universe.create_archetype("ViewportArch");
		universe.add_archetype_handler(arch.id(), Self::on_destroy);

		arch
	}
}

impl ViewportBundle {
	pub fn new(viewport: Viewport) -> Self {
		Self {
			viewport,
			input_manager: Default::default(),
			depth_texture: FullScreenTexture::new(
				"depth texture",
				wgpu::TextureFormat::Depth32Float,
				wgpu::TextureUsages::RENDER_ATTACHMENT,
			),
		}
	}

	fn on_destroy(universe: &mut Universe, events: EventQueueIter<EntityDestroyEvent>) {
		let mut guard;
		let mut cx = unpack!(&*universe => guard & (
			ResRw<&mut Storage<Viewport>>,
			ResRw<&mut Storage<InputManager>>,
			ResRw<&mut Storage<FullScreenTexture>>,
		));

		let arch_id = events.arch();
		let mut arch = universe.archetype_by_id(arch_id).lock();

		for (target, _) in events {
			let Self {
				viewport,
				depth_texture,
				input_manager,
			} = Self::detach(decompose!(cx), target);

			drop(input_manager);
			drop(depth_texture);
			drop(viewport);

			arch.despawn(target);
		}
	}
}
