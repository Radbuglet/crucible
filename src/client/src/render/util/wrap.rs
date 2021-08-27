//! Wrappers around Vulkan objects which cache their properties and provide some additional
//! helper methods.

use crate::render::core::VkContext;
use crate::render::vk_prelude::*;
use std::fmt::{Display, Formatter, Result as FmtResult};
use std::hash::Hash;

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
		write!(
			f,
			"{}x{}.{}.{}",
			self.variant, self.major, self.minor, self.patch
		)
	}
}

pub trait VkHandleWrapper {
	type Handle: Eq + Hash + Copy;

	fn handle(&self) -> Self::Handle;
}

#[derive(Debug, Clone)]
pub struct VkQueue {
	pub queue: vk::Queue,
	pub family: u32,
}

impl VkHandleWrapper for VkQueue {
	type Handle = vk::Queue;

	fn handle(&self) -> Self::Handle {
		self.queue
	}
}

#[derive(Debug, Clone)]
pub struct VkSurface {
	pub surface: vk::SurfaceKHR,
	pub caps: vk::SurfaceCapabilitiesKHR,
	pub present_modes: Vec<vk::PresentModeKHR>,
	pub formats: Vec<vk::SurfaceFormatKHR>,
}

impl VkSurface {
	pub unsafe fn new(cx: &VkContext, surface: vk::SurfaceKHR) -> anyhow::Result<Self> {
		let caps = cx
			.instance
			.get_physical_device_surface_capabilities_khr(cx.physical, surface)
			.result()?;

		let present_modes = cx
			.instance
			.get_physical_device_surface_present_modes_khr(cx.physical, surface, None)
			.result()?;

		let formats = cx
			.instance
			.get_physical_device_surface_formats_khr(cx.physical, surface, None)
			.result()?;

		Ok(Self {
			surface,
			caps,
			present_modes,
			formats,
		})
	}
}

impl VkHandleWrapper for VkSurface {
	type Handle = vk::SurfaceKHR;

	fn handle(&self) -> Self::Handle {
		self.surface
	}
}
