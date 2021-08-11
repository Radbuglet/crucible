use crate::util::str::strcmp;
use prelude::*;
use std::fmt::{Display, Formatter, Result as FmtResult};
use std::os::raw::c_char;

pub mod prelude {
	pub use erupt::{
		utils as vk_ext, vk, DeviceLoader as VkDevice, EntryLoader as VkEntry,
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
		write!(
			f,
			"{}x{}.{}.{}",
			self.variant, self.major, self.minor, self.patch
		)
	}
}

pub fn missing_set<'a, F, A, B>(
	equals: &'a F,
	set_a: &'a [A],
	set_b: &'a [B],
) -> impl Iterator<Item = &'a A> + 'a
where
	F: Fn(&'a A, &'a B) -> bool,
{
	set_a
		.iter()
		.filter(move |a| set_b.iter().find(move |b| equals(a, b)).is_none())
}

pub unsafe fn missing_extensions<'a>(
	required: &'a [*const c_char],
	present: &'a [vk::ExtensionProperties],
) -> impl Iterator<Item = &'a *const c_char> {
	missing_set::<_, *const c_char, vk::ExtensionProperties>(
		&|a, b| strcmp(*a, b.extension_name.as_ptr()),
		required,
		present,
	)
}
