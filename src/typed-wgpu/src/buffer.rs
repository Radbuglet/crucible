use derive_where::derive_where;

use crate::util::transparent_wrapper;

transparent_wrapper! {
	pub struct Buffer(wgpu::Buffer);

	#[derive_where(Copy, Clone)]
	pub struct BufferSlice<'a>(wgpu::BufferSlice<'a>);
}
