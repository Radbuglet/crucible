//! Utilities for Erupt FFI

use prelude::*;
use std::fmt::{Display, Formatter, Result as FmtResult};

pub mod prelude {
    pub use erupt::{
        utils as vk_utils, vk, DeviceLoader as VkDevice, EntryLoader as VkEntry,
        InstanceLoader as VkInstance,
    };
}

#[derive(Copy, Clone)]
pub struct VkVersion {
    pub variant: u32,
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl VkVersion {
    pub fn new(variant: u32, major: u32, minor: u32, patch: u32) -> Self {
        Self {
            variant,
            major,
            minor,
            patch,
        }
    }

    pub fn unpack(version: u32) -> Self {
        Self {
            variant: vk::api_version_variant(version),
            major: vk::api_version_major(version),
            minor: vk::api_version_minor(version),
            patch: vk::api_version_patch(version),
        }
    }

    pub fn pack(&self) -> u32 {
        vk::make_api_version(self.variant, self.major, self.minor, self.patch)
    }
}

impl Display for VkVersion {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_fmt(format_args!(
            "{}x{}.{}.{}",
            self.variant, self.major, self.minor, self.patch
        ))
    }
}
