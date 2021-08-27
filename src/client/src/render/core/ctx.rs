use crate::render::core::util::ffi::missing_extensions;
use crate::render::core::util::wrap::VkVersion;
use crate::render::core::vk_prelude::*;
use crate::util::error::{AnyResult, ResultContext};
use crate::util::str::*;
use anyhow::Context;
use winit::window::Window;

// TODO: Standardize allocator

/// A fully initialized Vulkan context.
pub struct VkContext {
	pub entry: VkEntry,
	pub instance: VkInstance,
	pub device: VkDevice,
	pub physical: vk::PhysicalDevice,
	pub render_queue: Queue,
	pub present_queue: Queue,
}

/// A Vulkan queue with its owning family index.
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct Queue {
	pub queue: vk::Queue,
	pub family: u32,
}

impl VkContext {
	pub fn new(main_window: &Window) -> AnyResult<(Box<Self>, vk::SurfaceKHR)> {
		unsafe {
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
					&mut vk_ext::surface::enumerate_required_extensions(main_window)
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
			let surface = vk_ext::surface::create_surface(&instance, main_window, None)
				.result()
				.context("Failed to create surface (OOM?)")?;

			// Find a suitable Vulkan implementation.
			struct PhysicalCandidate {
				physical: vk::PhysicalDevice,
				props: vk::PhysicalDeviceProperties,
				score: u32,
				render_queue_family: u32,
				present_queue_family: u32,
			}

			impl PhysicalCandidate {
				pub fn is_uniform(&self) -> bool {
					self.render_queue_family == self.present_queue_family
				}
			}

			let mandatory_exts = vec![];
			let physical = {
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

					candidates.push(PhysicalCandidate {
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

			// Create device and its queues
			let (device, render_queue, present_queue) = {
				// Collect queue create infos
				let queue_create_infos = if physical.is_uniform() {
					vec![vk::DeviceQueueCreateInfoBuilder::new()
						.queue_family_index(physical.render_queue_family)
						.queue_priorities(&[1.0])]
				} else {
					vec![
						vk::DeviceQueueCreateInfoBuilder::new()
							.queue_family_index(physical.render_queue_family)
							.queue_priorities(&[1.0]),
						vk::DeviceQueueCreateInfoBuilder::new()
							.queue_family_index(physical.present_queue_family)
							.queue_priorities(&[1.0]),
					]
				};

				// Create the device
				let device = VkDevice::new(
					&instance,
					physical.physical,
					&vk::DeviceCreateInfoBuilder::new()
						.enabled_extension_names(&mandatory_exts)
						.queue_create_infos(&queue_create_infos),
					None,
				)
				.context("Failed to create logical device (OOM?)")?;

				// Collect and wrap queues
				let (render_queue, present_queue) = if physical.is_uniform() {
					let queue = device.get_device_queue(physical.render_queue_family, 0);
					(queue, queue)
				} else {
					(
						device.get_device_queue(physical.render_queue_family, 0),
						device.get_device_queue(physical.present_queue_family, 0),
					)
				};

				// Return
				(
					device,
					Queue {
						queue: render_queue,
						family: physical.render_queue_family,
					},
					Queue {
						queue: present_queue,
						family: physical.present_queue_family,
					},
				)
			};

			// Construct DeviceManager
			Ok((
				Box::new(Self {
					entry,
					instance,
					device,
					physical: physical.physical,
					render_queue,
					present_queue,
				}),
				surface,
			))
		}
	}
}
