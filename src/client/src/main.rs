#![feature(never_type)]

use crate::util::error::AnyResult;
use crate::util::erupt::VkVersion;
use crate::util::erupt_prelude::*;
use crate::util::str::*;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;

mod util;

fn main() -> AnyResult<!> {
    unsafe {
        // Setup windows
        let event_loop = EventLoop::new();
        let main_window = WindowBuilder::new()
            .with_title("Crucible")
            .with_inner_size(LogicalSize::new(1920, 1080))
            .with_resizable(false)
            .with_visible(false)
            .build(&event_loop)?;

        let entry = VkEntry::new()?;

        // Print some debug information
        println!(
            "Using Vulkan version {}",
            entry
                .enumerate_instance_version()
                .result()
                .map(|version| VkVersion::unpack(version).to_string())
                .unwrap_or("<Failed to fetch version>".to_string())
        );

        // Create instance
        let instance = {
            // Collect extensions
            let extensions =
                vk_utils::surface::enumerate_required_extensions(&main_window).result()?;

            // Create instance
            VkInstance::new(
                &entry,
                &vk::InstanceCreateInfoBuilder::new()
                    .application_info(
                        &vk::ApplicationInfoBuilder::new()
                            .application_name(str_to_cstr("Crucible\0"))
                            .application_version(VkVersion::new(0, 0, 1, 0).pack()),
                    )
                    .enabled_extension_names(&extensions)
                    .enabled_layer_names(&[
                        // static_cstr!("VK_LAYER_LUNARG_api_dump"),
                        static_cstr!("VK_LAYER_KHRONOS_validation"),
                    ]),
                None,
            )?
        };

        // Create surface
        let surface = vk_utils::surface::create_surface(&instance, &main_window, None).result()?;

        // Find suitable GPU
        let (physical, present_queue_family) = {
            // Select a physical device
            let physical = instance.enumerate_physical_devices(None).result()?[0];
            let physical_props = instance.get_physical_device_properties(physical);

            println!(
                "Selected physical device {} running driver v{}",
                strbuf_to_str(&physical_props.device_name),
                VkVersion::unpack(physical_props.driver_version)
            );

            // Find family indices
            let (family_idx, _) = instance
                .get_physical_device_queue_family_properties(physical, None)
                .iter()
                .enumerate()
                .map(|(index, family)| (index as u32, family))
                .find(|(index, _family)| {
                    instance
                        .get_physical_device_surface_support_khr(physical, *index, surface)
                        .result()
                        .unwrap()
                })
                .expect("Failed to find a suitable family.");

            (physical, family_idx)
        };

        // Create device
        let (device, present_queue) = {
            // Collect extensions
            let extensions = [static_cstr!("VK_KHR_swapchain")];

            // Create device
            let device = VkDevice::new(
                &instance,
                physical,
                &vk::DeviceCreateInfoBuilder::new()
                    .queue_create_infos(&[vk::DeviceQueueCreateInfoBuilder::new()
                        .queue_family_index(present_queue_family)
                        .queue_priorities(&[1.0])])
                    .enabled_extension_names(&extensions),
                None,
            )?;

            // Resolve queues
            let queue = device.get_device_queue(present_queue_family, 0);

            // Pack
            (device, queue)
        };

        // Create swapchain
        let (swapchain, swapchain_images) = {
            // Identify valid usage
            let surface_tex_fmt = instance
                .get_physical_device_surface_formats_khr(physical, surface, None)
                .result()?[0];

            println!(
                "Using swapchain image configuration: {:?} in colorspace {:?}.",
                surface_tex_fmt.format, surface_tex_fmt.color_space
            );

            let surface_caps = instance
                .get_physical_device_surface_capabilities_khr(physical, surface)
                .result()?;

            let extent = surface_caps.current_extent;
            if extent.width == u32::MAX && extent.height == u32::MAX {
                panic!("Special value thingy"); // TODO: What?
            }

            println!("Current extent: {:?}", extent);

            let present_mode = instance
                .get_physical_device_surface_present_modes_khr(physical, surface, None)
                .result()?[0];

            println!("Present mode: {:?}", present_mode);

            let min_image_count = match present_mode {
                vk::PresentModeKHR::SHARED_DEMAND_REFRESH_KHR
                | vk::PresentModeKHR::SHARED_CONTINUOUS_REFRESH_KHR => 1,
                _ => surface_caps.min_image_count,
            };

            // Create swapchain
            let swapchain = device
                .create_swapchain_khr(
                    &vk::SwapchainCreateInfoKHRBuilder::new()
                        .surface(surface)
                        .min_image_count(min_image_count)
                        .image_format(surface_tex_fmt.format)
                        .image_color_space(surface_tex_fmt.color_space)
                        .image_extent(extent)
                        .image_array_layers(1)
                        .image_usage(vk::ImageUsageFlags::default())
                        .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
                        .pre_transform(surface_caps.current_transform)
                        .composite_alpha(vk::CompositeAlphaFlagBitsKHR::OPAQUE_KHR) // FIXME
                        .present_mode(present_mode),
                    None,
                )
                .result()?;

            // Collect images
            let images = device.get_swapchain_images_khr(swapchain, None)
                .result()?;

            // Pack
            (swapchain, images)
        };

        // Transition swapchain image formats
        {
            let pool = device.create_command_pool(
                &vk::CommandPoolCreateInfoBuilder::new()
                    .queue_family_index(present_queue_family),
                None,
            ).result()?;

            let command = device.allocate_command_buffers(
                &vk::CommandBufferAllocateInfoBuilder::new()
                    .level(vk::CommandBufferLevel::PRIMARY)
                    .command_pool(pool)
                    .command_buffer_count(1)
            ).result()?[0];

            device.begin_command_buffer(command, &vk::CommandBufferBeginInfoBuilder::new()).result()?;

            for image in &swapchain_images {
                device.cmd_pipeline_barrier(
                    command,
                    /* source */ vk::PipelineStageFlags::TOP_OF_PIPE,
                    /* dest   */ vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                    /* flags  */ None,
                    /* memory */ &[],
                    /* buffer */ &[],
                    /* image  */ &[
                        vk::ImageMemoryBarrierBuilder::new()
                            .src_access_mask(vk::AccessFlags::MEMORY_READ)  // TODO: What?
                            .dst_access_mask(vk::AccessFlags::MEMORY_READ)
                            .old_layout(vk::ImageLayout::UNDEFINED)
                            .new_layout(vk::ImageLayout::PRESENT_SRC_KHR)
                            .src_queue_family_index(present_queue_family)
                            .dst_queue_family_index(present_queue_family)
                            .image(*image)
                            .subresource_range(vk::ImageSubresourceRangeBuilder::new()
                                .aspect_mask(vk::ImageAspectFlags::COLOR)
                                .base_mip_level(0)
                                .level_count(vk::REMAINING_MIP_LEVELS)
                                .base_array_layer(0)
                                .layer_count(vk::REMAINING_ARRAY_LAYERS)
                                .build())
                    ],
                );
            }

            device.end_command_buffer(command).result()?;
            device.queue_submit(present_queue, &[
                vk::SubmitInfoBuilder::new()
                    .command_buffers(&[command])
            ], None).result()?;
            device.queue_wait_idle(present_queue).result()?;
            device.destroy_command_pool(Some(pool), None);
        }

        // Create present semaphores
        let image_ready = device.create_semaphore(&vk::SemaphoreCreateInfoBuilder::new(), None)
            .result()?;

        // Start main loop
        println!("Main loop started!");

        main_window.set_visible(true);
        event_loop.run(move |event, _proxy, flow| match &event {
            // Close requested
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *flow = ControlFlow::Exit,

            // Keypress
            Event::WindowEvent {
                event:
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                virtual_keycode: Some(VirtualKeyCode::Escape),
                                state: ElementState::Pressed,
                                ..
                            },
                        ..
                    },
                ..
            } => *flow = ControlFlow::Exit,

            // Render
            Event::RedrawRequested(_) => {
                // TODO: Is continually polling this legal? Who knows!
                let frame_idx = device.acquire_next_image_khr(swapchain, u64::MAX, Some(image_ready), None)
                    .result().unwrap();

                device.queue_present_khr(present_queue, &vk::PresentInfoKHRBuilder::new()
                    .wait_semaphores(&[image_ready])
                    .swapchains(&[swapchain])
                    .image_indices(&[frame_idx]))
                    .result().unwrap();
            }

            // Shutdown
            Event::LoopDestroyed => {
                println!("Shutting down...");
                let _ = device.queue_wait_idle(present_queue);
                device.destroy_semaphore(Some(image_ready), None);
                device.destroy_swapchain_khr(Some(swapchain), None);
                device.destroy_device(None);
                instance.destroy_surface_khr(Some(surface), None);
                instance.destroy_instance(None);
                println!("Goodbye!");
            }

            _ => {}
        });
    }
}
