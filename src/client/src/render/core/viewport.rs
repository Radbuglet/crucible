//! Viewport (surface, swapchain, frames and their views) managers.
//!
//! Useful resources:
//!
//! - [The xplain blog series](https://magcius.github.io/xplain/article/x-basics.html), which gives
//!   a nice overview of how the x11 windowing system and compositor work.
//! - The Vulkan API reference
//!

use crate::render::core::vk_prelude::*;
use crate::render::core::VkContext;
use winit::window::Window;

#[derive(Debug, Clone)]
pub struct ViewportConfig {
	/// A list of preferences for which present mode to use. Default to `FIFO` if none of the
	/// preferences can be accommodated.
	present_mode_prefs: Vec<vk::PresentModeKHR>,

	/// Indicates whether or not the presentation engine can give clipped (obscured) pixels to
	/// another window, which may prevent the implementation from reading back the entire frame
	/// buffer. Since we're not doing anything crazy with the swapchain, we can probably set this
	/// to true.
	compositor_can_clip: bool,

	/// Specifies how the compositor should blend transparency. We typically only need opaque
	/// compositing unless we want to do some neat UI tricks (e.g. a glass look with the `INHERIT`
	/// flag, which would require some platform-dependent blending hints).
	compositor_blend: vk::CompositeAlphaFlagsKHR,
}

pub struct Viewport {
	window: Window,
	surface: vk::SurfaceKHR,
	surface_caps: vk::SurfaceCapabilitiesKHR,
	surface_present_modes: Vec<vk::PresentModeKHR>,
	config: ViewportConfig,
	presenter: Option<Presenter>,
}

impl Viewport {
	pub unsafe fn from_parts(
		config: ViewportConfig,
		cx: &VkContext,
		window: &Window,
		surface: vk::SurfaceKHR,
	) -> anyhow::Result<Self> {
		todo!()
	}
}

struct Presenter {
	swapchain: vk::SwapchainKHR,
	frame_images: Vec<vk::Image>,
	frame_views: Vec<vk::ImageView>,
}

impl Presenter {
	pub unsafe fn new(
		config: &ViewportConfig,
		cx: &VkContext,
		surface: vk::SurfaceKHR,
		replaced: Option<vk::SwapchainKHR>,
	) -> anyhow::Result<Option<Self>> {
		todo!()
	}
}
