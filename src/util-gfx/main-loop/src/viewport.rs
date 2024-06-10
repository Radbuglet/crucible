use std::sync::Arc;

use bevy_autoken::{random_component, Obj, ObjOwner, RandomAccess, RandomEntityExt};
use bevy_ecs::removal_detection::RemovedComponents;
use hash_utils::FxHashMap;
use thiserror::Error;
use typed_glam::glam::UVec2;
use winit::window::{Window, WindowId};

use crate::GfxContext;

pub const FALLBACK_SURFACE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;

// === ViewportManager === //

#[derive(Debug, Default)]
pub struct ViewportManager {
    window_map: FxHashMap<WindowId, Obj<Viewport>>,
}

random_component!(ViewportManager);

impl ViewportManager {
    pub fn register(mut self: Obj<Self>, mut viewport: Obj<Viewport>) {
        viewport.manager = Some(self);
        self.window_map.insert(viewport.window.id(), viewport);
    }

    pub fn get_viewport(&self, id: WindowId) -> Option<Obj<Viewport>> {
        self.window_map.get(&id).copied()
    }

    pub fn window_map(&self) -> &FxHashMap<WindowId, Obj<Viewport>> {
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
    window: Arc<Window>,
    manager: Option<Obj<ViewportManager>>,
    surface: wgpu::Surface<'static>,
    curr_config: wgpu::SurfaceConfiguration,
    next_config: wgpu::SurfaceConfiguration,
    config_dirty: bool,
}

random_component!(Viewport);

impl Viewport {
    pub fn new(
        gfx: &GfxContext,
        window: Arc<Window>,
        surface: Option<wgpu::Surface<'static>>,
        config: wgpu::SurfaceConfiguration,
    ) -> Self {
        let surface =
            surface.unwrap_or_else(|| gfx.instance.create_surface(window.clone()).unwrap());

        Self {
            window,
            manager: None,
            surface,
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

    pub fn get_current_texture(
        &mut self,
        gfx: &GfxContext,
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
            let supported_formats = surface.get_capabilities(&gfx.adapter).formats;

            assert!(
                !supported_formats.is_empty(),
                "The current graphics adapter does not support this surface."
            );

            if config.format != FALLBACK_SURFACE_FORMAT
                && !supported_formats.contains(&config.format)
            {
                tracing::warn!(
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
                tracing::warn!(
                    "Request to acquire swap-chain for window {:?} timed out.",
                    self.window.id()
                );
                Ok(None)
            }
            Err(OutOfMemory) => Err(OutOfDeviceMemoryError),
            Err(Outdated) | Err(Lost) => {
                tracing::warn!(
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
                        tracing::warn!(
							"Failed to acquire swap-chain for window {:?} after swap-chain was recreated.",
							self.window.id()
						);
                        Ok(None)
                    }
                }
            }
        }
    }

    pub fn manager(&self) -> Option<Obj<ViewportManager>> {
        self.manager
    }
}

#[derive(Debug, Copy, Clone, Error)]
#[error("out of device memory")]
pub struct OutOfDeviceMemoryError;

// === Systems === //

pub fn sys_unregister_dead_viewports(
    mut rand: RandomAccess<(&mut ViewportManager, &Viewport)>,
    mut query: RemovedComponents<ObjOwner<Viewport>>,
) {
    rand.provide(|| {
        for viewport in query.read() {
            let viewport = viewport.get::<Viewport>();
            let Some(mut vmgr) = viewport.manager() else {
                continue;
            };

            vmgr.unregister(viewport.window().id());
        }
    })
}
