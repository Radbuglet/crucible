use crate::util::error::{AnyResult, ResultContext};
use crate::util::str::*;
use crate::util::vk_prelude::*;
use crate::util::vk_util::{missing_extensions, VkVersion};
use anyhow::Context;
use winit::dpi::LogicalSize;
use winit::event_loop::EventLoop;
use winit::window::WindowBuilder;

/// The core Vulkan rendering monolith. Implements:
///
/// - Instance and device creation
/// - Graphics queue acquisition and dispatch
/// - Window management and rendering
/// - The main loop
/// - Asset management (shaders, pipelines)
/// - Object management (allocation, automatic destruction)
///
/// Eventually, I'd like to split this up into several smaller resources and make detection logic
/// less strongly tied to Crucible's requirements. However, designing such abstractions without real
/// experience with the API is doomed to fail so I'm just going to sweep away all the ugly Vulkan
/// logic under the `GfxManager` rug.
pub struct GfxManager {
	// Vulkan singletons
	entry: VkEntry,
	instance: VkInstance,
	device: VkDevice,
}

impl GfxManager {
	// TODO: This might run into stack size constraints unless we box things.
	pub fn new() -> AnyResult<Box<Self>> {
		unsafe {
			// Create main window.
			// This will be used to query presentation support.
			let event_loop = EventLoop::new();
			let main_window = WindowBuilder::new()
				.with_title("Crucible")
				.with_inner_size(LogicalSize::new(1920, 1080))
				.with_resizable(false)
				.with_visible(false)
				.build(&event_loop)
				.context("Failed to create main window.")?;

			// Load entry
			let entry = VkEntry::new().context("Failed to fetch Vulkan loader.")?;

			// Print Vulkan details
			println!(
				"Vulkan API version: {}",
				entry.enumerate_instance_version().result().map_or_else(
					|_| "<not available>".to_string(),
					|packed| VkVersion::unpack(packed).to_string()
				)
			);

			// Create instance
			let instance = {
				// Collect mandatory extensions
				// We check extension presence in the instance error handler since Vulkan checks them
				// anyways and we don't want to duplicate work in the fast path.
				let mut mandatory_exts = vec![vk::KHR_SURFACE_EXTENSION_NAME];
				mandatory_exts.append(
					&mut vk_ext::surface::enumerate_required_extensions(&main_window)
						.result()
						.context("Failed to fetch required surface creation extensions.")?,
				);

				// Try to build instance
				// We don't enable any layers here since lunarg's Vulkan loader provides some really
				// nice user-facing layer selection mechanisms already.
				VkInstance::new(
					&entry,
					&vk::InstanceCreateInfoBuilder::new()
						.application_info(
							&vk::ApplicationInfoBuilder::new()
								.application_name(static_cstr!("Crucible")) // TODO: Load from Cargo
								.application_version(VkVersion::new(0, 0, 1, 0).pack())
								.api_version(vk::API_VERSION_1_0),
						)
						.enabled_extension_names(&mandatory_exts),
					None,
				)
				.with_context_proc(|err| {
					let mut builder = "Failed to create instance: ".to_string();

					// Handle link error & push primary reason
					let err = match err {
						erupt::LoaderError::SymbolNotAvailable => {
							builder.push_str("invalid dynamic library");
							return builder;
						}
						erupt::LoaderError::VulkanError(err) => {
							builder.push_str(&err.to_string());
							*err
						}
					};

					// Annotate specific reasons
					if err == vk::Result::ERROR_EXTENSION_NOT_PRESENT {
						match entry
							.enumerate_instance_extension_properties(None, None)
							.ok()
						{
							Some(present) => {
								builder.push_str("\nMissing extensions: ");
								builder.push_str(
									format_list(
										missing_extensions(&mandatory_exts, &present)
											.map(|strptr| strptr_to_str(*strptr)),
									)
									.as_str(),
								);
							}
							None => {
								builder.push_str("\nRequired extensions: ");
								builder.push_str(
									format_list(
										mandatory_exts.iter().map(|strptr| strptr_to_str(*strptr)),
									)
									.as_str(),
								);
							}
						}
					}

					builder
				})?
			};

			// Create main window's surface
			// We need to do this here to allow us to query for specific physical device capabilities.
			let surface = vk_ext::surface::create_surface(&instance, &main_window, None)
				.result()
				.context("Failed to create surface (OOM?)")?;

			// Find a suitable Vulkan implementation.
			struct PhysicalInfo {
				physical: vk::PhysicalDevice,
				props: vk::PhysicalDeviceProperties,
				score: u32,
				render_queue_family: u32,
				present_queue_family: u32,
			}

			let physical = {
				// Collect mandatory extensions.
				let mandatory_exts = vec![];

				// Filter all candidate implementations and annotate them with creation info.
				let mut candidates = Vec::new();

				let physicals = instance
					.enumerate_physical_devices(None)
					.result()
					.context("Failed to enumerate physical devices (OOM?)")?;

				println!(
					"Found {} physical device{}.",
					physicals.len(),
					if physicals.len() == 1 { "" } else { "s" }
				);

				for physical in &physicals {
					let physical = *physical;
					let props = instance.get_physical_device_properties(physical);

					// Check mandatory extension support
					{
						let extensions = instance
							.enumerate_device_extension_properties(physical, None, None)
							.result()
							.context("Failed to enumerate physical device extensions (OOM?)")?;

						let missing =
							missing_extensions(&mandatory_exts, &extensions).collect::<Vec<_>>();

						if !missing.is_empty() {
							println!(
								"\tRejected {}.\nMissing mandatory extensions: {}.\n",
								strbuf_to_str(&props.device_name),
								format_list(missing.iter().map(|strptr| strptr_to_str(**strptr)))
							);
							continue;
						}
					}

					// Find adequate queue families, short-circuiting if an optimal queue is found.
					let mut render_queue_family = None;
					let mut present_queue_family = None;

					let families =
						instance.get_physical_device_queue_family_properties(physical, None);

					for (family_idx, family_props) in families.iter().enumerate() {
						let family_idx = family_idx as u32;

						// Query support
						let supports_render =
							family_props.queue_flags.contains(vk::QueueFlags::GRAPHICS);

						let supports_present = instance
							.get_physical_device_surface_support_khr(
								physical,
								family_idx as u32,
								surface,
							)
							.result()
							.with_context(|| {
								format!("Failed to fetch surface support properties for physical device (OOM?)")
							})?;

						// Commit queue if applicable
						if supports_render {
							render_queue_family = Some(family_idx);
						}

						if supports_present {
							present_queue_family = Some(family_idx);
						}

						// Short-circuit if we found an optimal queue
						if supports_render && supports_present {
							break;
						}
					}

					// Reject physical device if it's missing a queue.
					if render_queue_family.is_none() || present_queue_family.is_none() {
						println!(
							"\tRejected {}.\nMissing a render or present queue.",
							strbuf_to_str(&props.device_name),
						);
						continue;
					}

					// Calculate candidate score
					let score = {
						// Define a ranking of physical devices
						let fallback_rank = 0;
						let mut ranking_table = [fallback_rank; 5];
						ranking_table[vk::PhysicalDeviceType::DISCRETE_GPU.0 as usize] = 4;
						ranking_table[vk::PhysicalDeviceType::INTEGRATED_GPU.0 as usize] = 3;
						ranking_table[vk::PhysicalDeviceType::CPU.0 as usize] = 2;
						ranking_table[vk::PhysicalDeviceType::VIRTUAL_GPU.0 as usize] = 1;

						// Calculate score
						ranking_table
							.get(props.device_type.0 as usize)
							.copied()
							.unwrap_or(fallback_rank)
					};

					// Commit candidate physical device
					println!(
						"\tFound candidate {} with score {}.",
						strbuf_to_str(&props.device_name),
						score
					);

					candidates.push(PhysicalInfo {
						physical,
						props,
						score,
						render_queue_family: render_queue_family.unwrap(),
						present_queue_family: present_queue_family.unwrap(),
					});
				}

				// Select candidate
				// TODO: Allow user to override selection
				candidates.sort_by(|a, b| a.score.cmp(&b.score));
				let selected = candidates
					.pop()
					.context("No valid physical devices found.")?;

				// Print out selected candidate
				println!(
					"Using physical device: {}",
					strbuf_to_str(&selected.props.device_name)
				);
				selected
			};

			// // Create swapchain
			// let (swapchain, swapchain_images) = {
			// 	// Identify valid usage
			// 	let surface_tex_fmt = instance
			// 		.get_physical_device_surface_formats_khr(physical, surface, None)
			// 		.result()?[0];
			//
			// 	println!(
			// 		"Using swapchain image configuration: {:?} in colorspace {:?}.",
			// 		surface_tex_fmt.format, surface_tex_fmt.color_space
			// 	);
			//
			// 	let surface_caps = instance
			// 		.get_physical_device_surface_capabilities_khr(physical, surface)
			// 		.result()?;
			//
			// 	let extent = surface_caps.current_extent;
			// 	if extent.width == u32::MAX && extent.height == u32::MAX {
			// 		panic!("Special value thingy"); // TODO: What?
			// 	}
			//
			// 	println!("Current extent: {:?}", extent);
			//
			// 	let present_mode = instance
			// 		.get_physical_device_surface_present_modes_khr(physical, surface, None)
			// 		.result()?[0];
			//
			// 	println!("Present mode: {:?}", present_mode);
			//
			// 	let min_image_count = match present_mode {
			// 		vk::PresentModeKHR::SHARED_DEMAND_REFRESH_KHR
			// 		| vk::PresentModeKHR::SHARED_CONTINUOUS_REFRESH_KHR => 1,
			// 		_ => surface_caps.min_image_count,
			// 	};
			//
			// 	// Create swapchain
			// 	let swapchain = device
			// 		.create_swapchain_khr(
			// 			&vk::SwapchainCreateInfoKHRBuilder::new()
			// 				.surface(surface)
			// 				.min_image_count(min_image_count)
			// 				.image_format(surface_tex_fmt.format)
			// 				.image_color_space(surface_tex_fmt.color_space)
			// 				.image_extent(extent)
			// 				.image_array_layers(1)
			// 				.image_usage(vk::ImageUsageFlags::default())
			// 				.image_sharing_mode(vk::SharingMode::EXCLUSIVE)
			// 				.pre_transform(surface_caps.current_transform)
			// 				.composite_alpha(vk::CompositeAlphaFlagBitsKHR::OPAQUE_KHR) // FIXME
			// 				.present_mode(present_mode),
			// 			None,
			// 		)
			// 		.result()?;
			//
			// 	// Collect images
			// 	let images = device.get_swapchain_images_khr(swapchain, None).result()?;
			//
			// 	// Pack
			// 	(swapchain, images)
			// };
			//
			// // Transition swapchain image formats
			// {
			// 	let pool = device
			// 		.create_command_pool(
			// 			&vk::CommandPoolCreateInfoBuilder::new()
			// 				.queue_family_index(present_queue_family),
			// 			None,
			// 		)
			// 		.result()?;
			//
			// 	let command = device
			// 		.allocate_command_buffers(
			// 			&vk::CommandBufferAllocateInfoBuilder::new()
			// 				.level(vk::CommandBufferLevel::PRIMARY)
			// 				.command_pool(pool)
			// 				.command_buffer_count(1),
			// 		)
			// 		.result()?[0];
			//
			// 	device
			// 		.begin_command_buffer(command, &vk::CommandBufferBeginInfoBuilder::new())
			// 		.result()?;
			//
			// 	for image in &swapchain_images {
			// 		device.cmd_pipeline_barrier(
			// 			command,
			// 			/* source */ vk::PipelineStageFlags::TOP_OF_PIPE,
			// 			/* dest   */ vk::PipelineStageFlags::BOTTOM_OF_PIPE,
			// 			/* flags  */ None,
			// 			/* memory */ &[],
			// 			/* buffer */ &[],
			// 			/* image  */
			// 			&[vk::ImageMemoryBarrierBuilder::new()
			// 				.src_access_mask(vk::AccessFlags::MEMORY_READ) // TODO: What?
			// 				.dst_access_mask(vk::AccessFlags::MEMORY_READ)
			// 				.old_layout(vk::ImageLayout::UNDEFINED)
			// 				.new_layout(vk::ImageLayout::PRESENT_SRC_KHR)
			// 				.src_queue_family_index(present_queue_family)
			// 				.dst_queue_family_index(present_queue_family)
			// 				.image(*image)
			// 				.subresource_range(
			// 					vk::ImageSubresourceRangeBuilder::new()
			// 						.aspect_mask(vk::ImageAspectFlags::COLOR)
			// 						.base_mip_level(0)
			// 						.level_count(vk::REMAINING_MIP_LEVELS)
			// 						.base_array_layer(0)
			// 						.layer_count(vk::REMAINING_ARRAY_LAYERS)
			// 						.build(),
			// 				)],
			// 		);
			// 	}
			//
			// 	device.end_command_buffer(command).result()?;
			// 	device
			// 		.queue_submit(
			// 			present_queue,
			// 			&[vk::SubmitInfoBuilder::new().command_buffers(&[command])],
			// 			None,
			// 		)
			// 		.result()?;
			// 	device.queue_wait_idle(present_queue).result()?;
			// 	device.destroy_command_pool(Some(pool), None);
			// }
			//
			// // Create present semaphores
			// let image_ready = device
			// 	.create_semaphore(&vk::SemaphoreCreateInfoBuilder::new(), None)
			// 	.result()?;
			//
			// // Finish
			// Ok(Box::new(Self {
			// 	entry,
			// 	instance,
			// 	device,
			// }))

			todo!()
		}
	}

	pub fn start(self) -> ! {
		todo!()
	}
}
