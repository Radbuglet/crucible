//! Utilities for Ash FFI

use std::fmt::{Display, Formatter, Result as FmtResult};

use self::prelude::*;

pub mod prelude {
    pub use ash::{
        extensions::experimental as vk_exp,
        extensions::ext as vk_ext,
        extensions::khr as vk_khr,
        extensions::mvk as vk_mvk,
        extensions::nv as vk_nv,
        version::{DeviceV1_0, EntryV1_0, InstanceV1_0},

        // Core handles and extensions
        vk,
        Device as VkDevice,
        // Core function loaders
        Entry as VkEntry,
        Instance as VkInstance,
    };
}

#[derive(Copy, Clone)]
pub struct VkVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl VkVersion {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    pub fn unpack(version: u32) -> Self {
        Self {
            major: vk::version_major(version),
            minor: vk::version_minor(version),
            patch: vk::version_patch(version),
        }
    }

    pub fn pack(&self) -> u32 {
        vk::make_version(self.major, self.minor, self.patch)
    }
}

impl Display for VkVersion {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_fmt(format_args!("{}.{}.{}", self.major, self.minor, self.patch))
    }
}
