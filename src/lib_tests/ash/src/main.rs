use std::ffi::c_void;
use std::os::raw::c_char;

use crate::util::ash::VkVersion;
use crate::util::ash_prelude::*;
use crate::util::error::AnyResult;
use crate::util::str::{str_to_strbuf, strbuf_to_str, strptr_to_str};

pub mod util;

fn main() -> AnyResult<()> {
    unsafe {
        let entry = VkEntry::new()?;

        // Print some debug information
        {
            println!(
                "Using Vulkan version {}",
                entry
                    .try_enumerate_instance_version()?
                    .map(|version| VkVersion::unpack(version).to_string())
                    .unwrap_or("<Failed to fetch version>".to_string())
            );

            println!("Instance extensions found:");
            for ext in entry.enumerate_instance_extension_properties()? {
                println!(
                    "\t- \"{}\" - v{}",
                    strbuf_to_str(&ext.extension_name),
                    VkVersion::unpack(ext.spec_version)
                );
            }

            println!("Layers found:");
            for layer in entry.enumerate_instance_layer_properties()? {
                println!(
                    "\t- \"{}\" - v{}\n\
                     \t  {}",
                    strbuf_to_str(&layer.layer_name),
                    VkVersion::unpack(layer.spec_version),
                    strbuf_to_str(&layer.description)
                );
            }
        }

        // Create instance
        let instance = {
            let extensions = vec![str_to_strbuf("VK_EXT_debug_report\0")];

            entry.create_instance(
                &vk::InstanceCreateInfo {
                    p_application_info: &vk::ApplicationInfo {
                        p_application_name: str_to_strbuf("Crucible\0"),
                        application_version: VkVersion::new(0, 1, 0).pack(),
                        ..Default::default()
                    },
                    pp_enabled_extension_names: extensions.as_ptr(),
                    enabled_extension_count: extensions.len() as u32,
                    ..Default::default()
                },
                None,
            )?
        };

        // Attach debug reporter
        {
            let instance_dbg = vk_ext::DebugReport::new(&entry, &instance);
            let _handle = instance_dbg.create_debug_report_callback(
                &vk::DebugReportCallbackCreateInfoEXT {
                    pfn_callback: Some(report_debug_msg),
                    ..Default::default()
                },
                None,
            )?;
        }

        // Select a physical device
        // TODO

        // Cleanup & shutdown
        instance.destroy_instance(None);
        println!("Goodbye!");
        Ok(())
    }
}

extern "system" fn report_debug_msg(
    flags: vk::DebugReportFlagsEXT,
    _object_type: vk::DebugReportObjectTypeEXT,
    _object: u64,
    _location: usize,
    message_code: i32,
    p_layer_prefix: *const c_char,
    p_message: *const c_char,
    _p_user_data: *mut c_void,
) -> vk::Bool32 {
    let prefix = match flags {
        vk::DebugReportFlagsEXT::INFORMATION => "INFO".to_string(),
        vk::DebugReportFlagsEXT::DEBUG => "DEBUG".to_string(),
        vk::DebugReportFlagsEXT::WARNING => "WARN".to_string(),
        vk::DebugReportFlagsEXT::ERROR => "ERROR".to_string(),
        vk::DebugReportFlagsEXT::PERFORMANCE_WARNING => "PERF".to_string(),
        _ => format!("UNKNOWN-{}", flags.as_raw()),
    };

    println!(
        "[{}] ({} - {}): {}",
        prefix,
        unsafe { strptr_to_str(p_layer_prefix) },
        message_code,
        unsafe { strptr_to_str(p_message) }
    );

    vk::TRUE
}
