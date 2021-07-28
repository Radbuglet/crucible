use std::ffi::CString;
use vulkano::app_info_from_cargo_toml;
use vulkano::device::Device;
use vulkano::device::DeviceExtensions;
use vulkano::device::Features;
use vulkano::instance::debug::{DebugCallback, MessageSeverity, MessageType};
use vulkano::instance::{Instance, InstanceExtensions, PhysicalDevice, QueueFamily};
use vulkano::Version;
use vulkano_win::{create_vk_surface, required_extensions};
use winit::dpi::LogicalSize;
use winit::event_loop::EventLoop;
use winit::window::WindowBuilder;

type AnyResult<T> = Result<T, Box<dyn std::error::Error>>;

fn main() -> AnyResult<()> {
    // Create window
    println!("Creating window...");
    let event_loop = EventLoop::new();
    let main_window = WindowBuilder::new()
        .with_title("Crucible")
        .with_inner_size(LogicalSize::new(1920, 1080))
        .with_visible(false)
        .build(&event_loop)?;

    // Create instance
    println!("Initializing Vulkan...");
    let instance = {
        let none = InstanceExtensions::none();
        let supported = InstanceExtensions::supported_by_core()?;

        // Collect mandatory extensions
        let extensions = InstanceExtensions {
            ext_debug_utils: true,
            ..none
        };
        let extensions = extensions.union(&required_extensions());

        // Panic on missing extensions
        if extensions.difference(&supported) != none {
            panic!("Missing required InstanceExtension(s).");
        }

        // Collect optional extensions
        // TODO

        // Print enabled extension(s)
        println!("Extensions requested:");
        for ext_name in Vec::<CString>::from(&extensions) {
            println!(
                "\t- {}",
                ext_name.to_str().unwrap_or("<failed to marshall>")
            );
        }

        // Build instance
        Instance::new(
            Some(&app_info_from_cargo_toml!()),
            Version::V1_1,
            &extensions,
            None,
        )?
    };
    println!("Created instance!");

    // Setup debug callbacks
    let _callback = DebugCallback::new(
        &instance,
        MessageSeverity::errors_and_warnings(),
        MessageType::all(),
        move |msg| {
            println!(
                "{}: {}",
                &msg.layer_prefix.unwrap_or("<any layer>"),
                &msg.description
            );
        },
    );

    // Create surface
    let surface = create_vk_surface(&main_window, instance.clone())?;

    // Find adequate physical device
    let physical = PhysicalDevice::enumerate(&instance)
        .next()
        .expect("No devices.");

    println!(
        "Selected physical device: {}",
        physical
            .properties()
            .device_name
            .as_ref()
            .map(String::as_str)
            .unwrap_or("<no name>")
    );

    // Select main queue family
    let family = physical
        .queue_families()
        .find(QueueFamily::supports_graphics)
        .expect("No valid queue families found.");

    // Create device
    let (device, queues) = Device::new(
        physical,
        &Features::none(),
        &DeviceExtensions::none(),
        [(family, 0.5)],
    )?;
    let queues = queues.collect::<Vec<_>>();

    println!("Created device!");

    // TODO: Do something cool

    Ok(())
}
