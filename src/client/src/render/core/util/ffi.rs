use crate::render::core::vk_prelude::*;
use crate::util::str::strcmp;
use std::os::raw::c_char;

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
