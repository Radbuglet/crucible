use crucible_util::{lang::marker::PhantomProlong, transparent};
use derive_where::derive_where;

transparent! {
	#[derive_where(Debug)]
	pub struct Buffer<T>(pub wgpu::Buffer, PhantomProlong<T>);

	#[derive_where(Debug, Copy, Clone)]
	pub struct BufferSlice<'a, T>(pub wgpu::BufferSlice<'a>, PhantomProlong<T>);

	#[derive_where(Debug, Clone)]
	pub struct BufferBinding<'a, T>(pub wgpu::BufferBinding<'a>, PhantomProlong<T>);
}
