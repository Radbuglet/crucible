use std::{fmt::Debug, hash::Hash, num::NonZeroU32};

use derive_where::derive_where;

use crate::{
	uniform::{UniformSet, UniformSetLayout},
	vertex::{VertexBufferSet, VertexShader},
};

// === Builder === //

mod sealed {
	use super::*;

	pub enum Unspecified {}

	pub trait UniformSetOrNever {
		type Config: 'static + Hash + Eq;
	}

	impl UniformSetOrNever for Unspecified {
		type Config = ();
	}

	impl<T: UniformSet> UniformSetOrNever for T {
		type Config = T::Config;
	}

	pub trait VertexBufferSetOrNever {
		type Config: 'static + Hash + Eq;
	}

	impl VertexBufferSetOrNever for Unspecified {
		type Config = ();
	}

	impl<T: VertexBufferSet> VertexBufferSetOrNever for T {
		type Config = T::Config;
	}
}

#[derive_where(Debug; U::Config: Debug, V::Config: Debug)]
#[derive_where(Default)]
pub struct RenderPipelineBuilder<'a, U = sealed::Unspecified, V = sealed::Unspecified>
where
	U: sealed::UniformSetOrNever,
	V: sealed::VertexBufferSetOrNever,
{
	// Debug config
	label: Option<&'a str>,

	// Shader config
	uniform_layout: Option<(&'a UniformSetLayout<U>, U::Config)>,
	vertex_shader: Option<(&'a VertexShader<V>, V::Config)>,
	fragment_shader: Option<wgpu::FragmentState<'a>>,

	// Fixed-function config
	primitive: wgpu::PrimitiveState,
	multisample: wgpu::MultisampleState,
	depth_stencil: Option<wgpu::DepthStencilState>,
	multiview: Option<NonZeroU32>,
}

impl RenderPipelineBuilder<'_> {
	pub fn new() -> Self {
		Self::default()
	}
}

impl<'a, U, V> RenderPipelineBuilder<'a, U, V>
where
	U: sealed::UniformSetOrNever,
	V: sealed::VertexBufferSetOrNever,
{
	pub fn with_label(mut self, label: &'a str) -> Self {
		self.label = Some(label);
		self
	}

	// TODO
}

// === Instance === //

pub struct RenderPipeline<U: UniformSet, V: VertexBufferSet> {
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
