use crate::engine::context::GfxContext;
use crate::engine::util::vec_ext::VecConvert;
use cgmath::Vector2;
use crucible_core::foundation::prelude::*;
use std::collections::HashMap;
use winit::window::{Window, WindowId};

pub const SWAPCHAIN_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;
pub const DEPTH_TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

#[derive(Default)]
pub struct ViewportManager {
	windows: HashMap<WindowId, Entity>,
	viewports: Storage<Viewport>,
}

impl ViewportManager {
	pub fn new() -> Self {
		Default::default()
	}

	pub fn register(&mut self, world: &World, gfx: &GfxContext, entity: Entity, window: Window) {
		let surface = unsafe { gfx.instance.create_surface(&window) };
		self.register_pair(world, gfx, entity, window, surface);
	}

	/// Constructs a viewport from the given `window` and `surface` and attaches it to the provided
	/// `entity`.
	pub fn register_pair(
		&mut self,
		world: &World,
		gfx: &GfxContext,
		entity: Entity,
		window: Window,
		surface: wgpu::Surface,
	) {
		let win_id = window.id();

		let old_viewport =
			self.viewports
				.insert(world, entity, Viewport::new(gfx, window, surface));

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
	pub fn get_entities(&self) -> impl Iterator<Item = Entity> + ExactSizeIterator + '_ {
		self.windows.values().copied()
	}

	/// Gets a viewport for a given `Entity`.
	pub fn get_viewport(&self, id: Entity) -> Option<ComponentPair<Viewport>> {
		self.viewports.try_get_raw(id)
	}

	/// Gets a viewport for a given `Entity`.
	pub fn get_viewport_mut(&mut self, id: Entity) -> Option<ComponentPairMut<Viewport>> {
		self.viewports.try_get_mut_raw(id)
	}

	/// Unregisters a viewport with a given `WindowId`.
	pub fn unregister(&mut self, id: WindowId) {
		let entity_id = self.windows.remove(&id).unwrap();
		self.viewports.remove(entity_id);
	}
}

#[derive(Debug)]
pub struct Viewport {
	window: Window,
	surface: wgpu::Surface,
	config: wgpu::SurfaceConfiguration,
}

impl Viewport {
	pub fn new(gfx: &GfxContext, window: Window, surface: wgpu::Surface) -> Self {
		let config = wgpu::SurfaceConfiguration {
			present_mode: wgpu::PresentMode::Fifo,
			format: SWAPCHAIN_FORMAT,
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
		let size = self.window.inner_size().to_vec();

		// We should never attempt to render to a zero-sized window.
		if !has_valid_size(size) {
			return None;
		}

		// Attempt to get a new frame from the current swapchain
		if size == Vector2::new(self.config.width, self.config.height) {
			match self.surface.get_current_texture() {
				// Desired outcome
				Ok(
					output @ wgpu::SurfaceTexture {
						suboptimal: false, ..
					},
				) => return Some(output),

				// Unrecoverable
				Err(wgpu::SurfaceError::Timeout) => return None,
				Err(wgpu::SurfaceError::OutOfMemory) => return None,

				// Try re-create
				Ok(wgpu::SurfaceTexture {
					suboptimal: true, ..
				}) => (),
				Err(wgpu::SurfaceError::Outdated) => (),
				Err(wgpu::SurfaceError::Lost) => (),
			}
		}

		// Re-create and try again
		log::info!("Recreating surface {:?}", self.surface);
		self.config.width = size.x;
		self.config.height = size.y;
		self.surface.configure(&gfx.device, &self.config);

		// Second attempt
		self.surface.get_current_texture().ok()
	}

	pub fn id(&self) -> WindowId {
		self.window.id()
	}

	pub fn window(&self) -> &Window {
		&self.window
	}

	pub fn aspect(&self) -> f32 {
		let size = self.window.inner_size().to_vec();
		if has_valid_size(size) {
			size.x as f32 / size.y as f32
		} else {
			1.
		}
	}
}

pub fn has_valid_size<T: cgmath::BaseNum>(size: Vector2<T>) -> bool {
	size.x != T::zero() && size.y != T::zero()
}

#[derive(Default)]
pub struct DepthTextureManager {
	attachments: Storage<DepthAttachment>,
}

impl DepthTextureManager {
	pub fn new() -> Self {
		Default::default()
	}

	pub fn register(&mut self, world: &World, gfx: &GfxContext, viewport: ComponentPair<Viewport>) {
		self.attachments.insert(
			world,
			viewport.entity_id(),
			DepthAttachment::new(gfx, viewport.component()),
		);
	}

	pub fn unregister(&mut self, id: Entity) {
		self.attachments
			.remove(id)
			.expect("Failed to unregister DepthTextureManager.");
	}

	pub fn present(
		&mut self,
		world: &World,
		gfx: &GfxContext,
		viewport: ComponentPair<Viewport>,
	) -> wgpu::TextureView {
		self.attachments
			.get_mut(world, viewport.entity_id())
			.present(gfx, viewport.component())
	}
}

struct DepthAttachment {
	texture: wgpu::Texture,
	size: Vector2<u32>,
}

impl DepthAttachment {
	pub fn new(gfx: &GfxContext, viewport: &Viewport) -> Self {
		let size = viewport.window().inner_size().to_vec();
		let texture = gfx.device.create_texture(&wgpu::TextureDescriptor {
			label: Some("depth texture"),
			size: wgpu::Extent3d {
				width: size.x,
				height: size.y,
				depth_or_array_layers: 1,
			},
			mip_level_count: 1,
			sample_count: 1,
			dimension: wgpu::TextureDimension::D2,
			format: DEPTH_TEXTURE_FORMAT,
			usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
		});
		Self { texture, size }
	}

	pub fn texture(&self) -> &wgpu::Texture {
		&self.texture
	}

	pub fn size(&self) -> Vector2<u32> {
		self.size
	}

	pub fn present(&mut self, gfx: &GfxContext, viewport: &Viewport) -> wgpu::TextureView {
		// Handle resizes
		let curr_size = viewport.window().inner_size().to_vec();
		if self.size != curr_size {
			*self = DepthAttachment::new(gfx, viewport);
		}

		// Create view
		self.texture.create_view(&Default::default())
	}
}
