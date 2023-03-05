use std::num::NonZeroU32;

use derive_where::derive_where;

use crate::{uniform::UniformSetLayout, vertex::VertexShader};

#[derive_where(Debug, Default)]
pub struct RenderPipelineBuilder<'a, U, V> {
	// Debug config
	label: Option<&'a str>,

	// Shader config
	uniform_layout: Option<&'a UniformSetLayout<U>>,
	vertex_shader: Option<&'a VertexShader<V>>,
	fragment_shader: Option<wgpu::FragmentState<'a>>,

	// Fixed-function config
	primitive: wgpu::PrimitiveState,
	multisample: wgpu::MultisampleState,
	depth_stencil: Option<wgpu::DepthStencilState>,
	multiview: Option<NonZeroU32>,
}
