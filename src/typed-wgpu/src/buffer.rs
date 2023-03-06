use crucible_util::transparent;
use derive_where::derive_where;

transparent! {
	#[derive_where(Debug)]
	pub struct Buffer<T>(pub wgpu::Buffer, T);

	#[derive_where(Debug, Copy, Clone)]
	pub struct BufferSlice<'a, T>(pub wgpu::BufferSlice<'a>, T);
}
