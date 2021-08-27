//! Viewport (surface, swapchain, frames and their views) managers.
//!
//! Useful resources:
//!
//! - [The xplain blog series](https://magcius.github.io/xplain/article/x-basics.html), which gives
//!   a nice overview of how the x11 windowing system and compositor work.
//! - [How To Configure Your Vulkan Swapchain](swapchain-howto), which gives a nice summary of a few
//!   Vulkan swapchain present modes.
//! - The Vulkan API reference
//!
//! [swapchain-howto]: https://developer.samsung.com/sdp/blog/en-us/2019/07/26/vulkan-mobile-best-practice-how-to-configure-your-vulkan-swapchain

use crate::render::core::VkContext;
use crate::render::util::ffi::UNSPECIFIED_CURRENT_EXTENT;
use crate::render::util::wrap::{VkHandleWrapper, VkSurface};
use crate::render::vk_prelude::*;
use crate::util::bitflag::choose_first_flag;
use crate::util::vector::{extent_to_vec2, vec2_to_extent, win_sz_to_vec2, VecExt};
use cgmath::num_traits::clamp;
use cgmath::Zero;
use winit::window::Window;

#[derive(Debug, Clone)]
pub struct ViewportConfig {
	/// A list of preferences for which present mode to use and the desired number of images to use
	/// with them.
	/// Default to triple-buffered `FIFO` (v-sync) if none of the preferences can be accommodated.
	pub present_mode_prefs: Vec<(vk::PresentModeKHR, u32)>,

	/// Indicates whether or not the presentation engine can give clipped (obscured) pixels to
	/// another window, which may prevent the implementation from reading back the entire frame
	/// buffer. Since we're not doing anything crazy with the swapchain, we can probably set this
	/// to true.
	pub compositor_can_clip: bool,

	/// Specifies how the compositor should blend transparency. We typically only need opaque
	/// compositing unless we want to do some neat UI tricks (e.g. a glass look with the `INHERIT`
	/// flag, which would require some platform-dependent blending hints).
	///
	/// If the desired mode is unsupported, the implementation will fall back to an arbitrary mode.
	/// In effect, this means that users should not rely on the `OPAQUE` mode being supported and
	/// should always specify appropriate alpha values for all situations.
	pub compositor_blend: vk::CompositeAlphaFlagsKHR,
}

pub struct Viewport {
	window: Window,
	surface: VkSurface,
	config: ViewportConfig,
	presenter: Option<Presenter>,
}

impl Viewport {
	pub unsafe fn from_parts(
		cx: &VkContext,
		config: ViewportConfig,
		window: Window,
		surface: VkSurface,
	) -> anyhow::Result<Self> {
		let mut viewport = Self {
			window,
			surface,
			config,
			presenter: None,
		};
		viewport.presenter = Presenter::new(cx, &viewport, None)?;

		Ok(viewport)
	}

	pub fn window(&self) -> &Window {
		&self.window
	}
}

struct Presenter {
	swapchain: vk::SwapchainKHR,
	images: Vec<vk::Image>,
	views: Vec<vk::ImageView>,
}

impl Presenter {
	pub unsafe fn new(
		cx: &VkContext,
		parent: &Viewport,
		replaced: Option<vk::SwapchainKHR>,
	) -> anyhow::Result<Option<Self>> {
		let window = &parent.window;
		let config = &parent.config;
		let surface = &parent.surface;

		// Detect settings
		let extent = {
			let min = extent_to_vec2(surface.caps.min_image_extent);
			let max = extent_to_vec2(surface.caps.max_image_extent);
			let curr_sz = extent_to_vec2(surface.caps.current_extent);
			let win_sz = win_sz_to_vec2(window.inner_size());

			let extent = if curr_sz != UNSPECIFIED_CURRENT_EXTENT {
				curr_sz
			} else {
				win_sz.clamp_comps(min, max)
			};

			if extent.is_zero() {
				return Ok(None);
			}

			vec2_to_extent(extent)
		};

		let (present_mode, image_bound) = {
			let (present_mode, requested_images) = config
				.present_mode_prefs
				.iter()
				.copied()
				.find(|(pref, _)| surface.present_modes.contains(&pref))
				// FIFO is the only mode with guaranteed support and is a reasonable default.
				.unwrap_or((vk::PresentModeKHR::FIFO_KHR, 3));

			let image_bound = match present_mode {
				// These settings are always valid.
				vk::PresentModeKHR::SHARED_DEMAND_REFRESH_KHR => 1,
				vk::PresentModeKHR::SHARED_CONTINUOUS_REFRESH_KHR => 1,

				// For everything else, we need to clamp then within the allowed image range.
				_ => {
					let req_min = surface.caps.min_image_count;
					let mut req_max = surface.caps.max_image_count;
					if req_max == 0 {
						req_max = u32::MAX;
					}

					clamp(requested_images, req_min, req_max)
				}
			};

			(present_mode, image_bound)
		};

		let composite_alpha = {
			let supported = surface.caps.supported_composite_alpha;
			let pref = config.compositor_blend & supported;
			let choice = if pref.is_empty() {
				choose_first_flag!(vk::CompositeAlphaFlagsKHR, supported)
			} else {
				pref
			};
			vk::CompositeAlphaFlagBitsKHR(choice.bits())
		};

		let format_choice = surface.formats.first().unwrap();

		// Create swapchain
		let swapchain = cx
			.device
			.create_swapchain_khr(
				&vk::SwapchainCreateInfoKHRBuilder::new()
					.surface(surface.handle())
					.min_image_count(image_bound)
					.image_format(format_choice.format)
					.image_color_space(format_choice.color_space)
					.image_extent(extent)
					.image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
					.image_sharing_mode(vk::SharingMode::EXCLUSIVE)
					.pre_transform(surface.caps.current_transform)
					.composite_alpha(composite_alpha)
					.present_mode(present_mode)
					.clipped(config.compositor_can_clip)
					.old_swapchain(replaced.unwrap_or(vk::SwapchainKHR::null())),
				None,
			)
			.result()?;

		// Collect images
		let images = cx
			.device
			.get_swapchain_images_khr(swapchain, None)
			.result()?;

		let mut views = Vec::with_capacity(images.len());

		for image in images.iter().copied() {
			let view = cx
				.device
				.create_image_view(
					&vk::ImageViewCreateInfoBuilder::new()
						.image(image)
						// TODO: We need some generic image format selection algorithm
						.format(vk::Format::R8G8B8A8_SRGB)
						.components(vk::ComponentMapping::default())
						.subresource_range(
							vk::ImageSubresourceRangeBuilder::new()
								.aspect_mask(vk::ImageAspectFlags::COLOR)
								.base_array_layer(0)
								.layer_count(0)
								.base_mip_level(0)
								.level_count(0)
								.build(),
						)
						.view_type(vk::ImageViewType::_2D),
					None,
				)
				.result()?;

			views.push(view);
		}

		// Construct Presenter
		Ok(Some(Self {
			swapchain,
			images,
			views,
		}))
	}
}
