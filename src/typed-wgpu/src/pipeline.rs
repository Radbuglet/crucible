use std::{marker::PhantomData, num::NonZeroU32};

use derive_where::derive_where;

use crate::{
	uniform::{UniformSet, UniformSetLayout},
	vertex::{VertexBufferSet, VertexShader},
};

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

pub struct RenderPipeline<U: UniformSet, V: VertexBufferSet> {
	pub ty: PhantomData<(U, V)>,
	pub uniform_config: U::Config,
	pub vertex_config: V::Config,
	pub raw: wgpu::RenderPipeline,
}

impl<U: UniformSet, V: VertexBufferSet> RenderPipeline<U, V> {
	pub fn bind_pipeline<'r>(&'r self, pass: &mut wgpu::RenderPass<'r>) {
		pass.set_pipeline(&self.raw);
	}

	pub fn bind_uniforms<'r>(&self, pass: &mut wgpu::RenderPass<'r>, uniforms: &'r U) {
		uniforms.apply_to_pass(pass, &self.uniform_config);
	}

	pub fn bind_vertex_set<'r>(&self, pass: &mut wgpu::RenderPass<'r>, vertices: &'r V) {
		vertices.apply_to_pass(&self.vertex_config, pass);
	}
}
