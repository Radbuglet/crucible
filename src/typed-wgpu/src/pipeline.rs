use std::{borrow::Cow, marker::PhantomData, num::NonZeroU32};

use crucible_util::{lang::marker::PhantomInvariant, transparent};
use derive_where::derive_where;

use crate::{
	uniform::{UniformSetInstanceGenerator, UniformSetKind, UniformSetLayout},
	vertex::{
		VertexBufferSetInstanceGenerator, VertexBufferSetKind, VertexBufferSetLayoutGenerator,
	},
};

// === RenderPipelineBuilder === //

#[derive_where(Default)]
pub struct RenderPipelineBuilder<'a, U = (), V = ()> {
	// Debug config
	label: Option<&'a str>,

	// Vertex shader config
	uniforms: Option<&'a UniformSetLayout<U>>,
	vertex_shader: Option<(
		&'a wgpu::ShaderModule,
		&'a str,
		Cow<'a, [wgpu::VertexBufferLayout<'a>]>,
		PhantomInvariant<V>,
	)>,
	fragment_shader: Option<(
		&'a wgpu::ShaderModule,
		&'a str,
		Cow<'a, [Option<wgpu::ColorTargetState>]>,
	)>,

	// Fixed function config
	primitive: wgpu::PrimitiveState,
	multisample: wgpu::MultisampleState,
	depth_stencil: Option<wgpu::DepthStencilState>,
	multiview: Option<NonZeroU32>,
}

impl<'a, U, V> RenderPipelineBuilder<'a, U, V> {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn with_label(mut self, label: &'a str) -> Self {
		self.label = Some(label);
		self
	}

	pub fn with_uniforms(mut self, set: &'a UniformSetLayout<U>) -> Self {
		self.uniforms = Some(set);
		self
	}

	pub fn with_vertex_shader(
		mut self,
		module: &'a wgpu::ShaderModule,
		entry: &'a str,
		buffers: &'a impl VertexBufferSetLayoutGenerator<Kind = V>,
	) -> Self {
		self.vertex_shader = Some((module, entry, buffers.layouts(), PhantomData));
		self
	}

	pub fn with_fragment_shader(
		self,
		module: &'a wgpu::ShaderModule,
		entry: &'a str,
		format: wgpu::TextureFormat,
	) -> Self {
		self.with_multi_fragment_shader(
			module,
			entry,
			vec![Some(wgpu::ColorTargetState {
				format,
				blend: None,
				write_mask: wgpu::ColorWrites::all(),
			})],
		)
	}

	pub fn with_multi_fragment_shader(
		mut self,
		module: &'a wgpu::ShaderModule,
		entry: &'a str,
		targets: impl Into<Cow<'a, [Option<wgpu::ColorTargetState>]>>,
	) -> Self {
		self.fragment_shader = Some((module, entry, targets.into()));
		self
	}

	pub fn with_vertex_topology(mut self, topology: wgpu::PrimitiveTopology) -> Self {
		self.primitive.topology = topology;
		self
	}

	pub fn with_index_strip_topology(mut self, strip_index_format: wgpu::IndexFormat) -> Self {
		self.primitive.strip_index_format = Some(strip_index_format);
		self
	}

	pub fn with_cw_front_face(mut self) -> Self {
		self.primitive.front_face = wgpu::FrontFace::Cw;
		self
	}

	pub fn with_cull_mode(mut self, mode: wgpu::Face) -> Self {
		self.primitive.cull_mode = Some(mode);
		self
	}

	pub fn with_unclipped_depth(mut self) -> Self {
		self.primitive.unclipped_depth = true;
		self
	}

	pub fn with_line_draw_mode(mut self) -> Self {
		self.primitive.polygon_mode = wgpu::PolygonMode::Line;
		self
	}

	pub fn with_point_draw_mode(mut self) -> Self {
		self.primitive.polygon_mode = wgpu::PolygonMode::Point;
		self
	}

	pub fn with_conservative_fill(mut self) -> Self {
		self.primitive.conservative = true;
		self
	}

	pub fn with_multisample(mut self, state: wgpu::MultisampleState) -> Self {
		self.multisample = state;
		self
	}

	pub fn with_depth_stencil(mut self, state: wgpu::DepthStencilState) -> Self {
		self.depth_stencil = Some(state);
		self
	}

	pub fn with_multiview(mut self, count: NonZeroU32) -> Self {
		self.multiview = Some(count);
		self
	}

	// TODO: More config stuff

	pub fn finish(self, device: &wgpu::Device) -> RenderPipeline<U, V> {
		device
			.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
				label: self.label,
				layout: self.uniforms.map(|u| &u.raw),
				vertex: {
					let (module, entry_point, buffers, _) = self
						.vertex_shader
						.as_ref()
						.expect("failed to create render pipeline: no vertex shader specified");

					wgpu::VertexState {
						module,
						entry_point,
						buffers,
					}
				},
				primitive: self.primitive,
				depth_stencil: self.depth_stencil,
				multisample: self.multisample,
				fragment: self
					.fragment_shader
					.as_ref()
					.map(|(module, entry_point, targets)| wgpu::FragmentState {
						module,
						entry_point,
						targets: &targets,
					}),
				multiview: self.multiview,
			})
			.into()
	}
}

// === RenderPipeline === //

transparent! {
	#[derive_where(Debug)]
	pub struct RenderPipeline<U, V>(pub wgpu::RenderPipeline, (U, V));
}

impl<U, V> RenderPipeline<U, V> {
	pub fn bind<'a>(
		&'a self,
		pass: &mut wgpu::RenderPass<'a>,
		uniforms: &'a impl UniformSetInstanceGenerator<U>,
		buffers: &'a impl VertexBufferSetInstanceGenerator<V>,
	) where
		U: UniformSetKind,
		V: VertexBufferSetKind,
	{
		self.bind_pipeline(pass);
		self.bind_uniforms(pass, uniforms);
		self.bind_vertex_buffers(pass, buffers);
	}

	pub fn bind_pipeline<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
		pass.set_pipeline(&self.raw);
	}

	pub fn bind_uniforms<'a>(
		&self,
		pass: &mut wgpu::RenderPass<'a>,
		uniforms: &'a impl UniformSetInstanceGenerator<U>,
	) where
		U: UniformSetKind,
	{
		uniforms.apply(pass);
	}

	pub fn bind_vertex_buffers<'a>(
		&self,
		pass: &mut wgpu::RenderPass<'a>,
		buffers: &'a impl VertexBufferSetInstanceGenerator<V>,
	) where
		V: VertexBufferSetKind,
	{
		buffers.apply(pass);
	}
}
