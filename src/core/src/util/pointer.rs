use std::alloc::Layout;

pub fn align_down(addr: usize, align: usize) -> usize {
	assert!(align.is_power_of_two());
	addr & !(align - 1)
}

pub fn align_down_offset(addr: usize, align: usize) -> usize {
	assert!(align.is_power_of_two());
	addr & (align - 1)
}

// TODO: How does one align addresses efficiently?!
pub fn align_up_offset(addr: usize, align: usize) -> usize {
	// Gets the offset to the lower address.
	let offset = align_down_offset(addr, align);

	// Take the offset required to align up and discard anything equal to `align`.
	(align - offset) & (align - 1)
}

pub fn align_up(addr: usize, align: usize) -> usize {
	assert!(align.is_power_of_two());
	// This will never overflow because we can align to at most the nearest usize::MAX boundary, and
	// all addresses are already naturally below or equal to this threshold.
	addr + align_up_offset(addr, align)
}

pub const fn layout_from_size_and_align(size: usize, align: usize) -> Layout {
	match Layout::from_size_align(size, align) {
		Ok(layout) => layout,
		Err(_) => panic!("Invalid layout!"),
	}
}
