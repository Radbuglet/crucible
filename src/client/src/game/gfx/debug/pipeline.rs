use bort::CompRef;
use crevice::std430::AsStd430;
use typed_glam::glam;
use typed_wgpu::{pipeline::RenderPipeline, vertex::VertexBufferLayout};

use crate::engine::{assets::AssetManager, io::gfx::GfxContext};

#[derive(Debug, AsStd430)]
pub struct WireframeVertex {
	pub pos: glam::Vec3,
	pub color: glam::Vec3,
}

impl WireframeVertex {
	pub fn layout() -> VertexBufferLayout<Self> {
		VertexBufferLayout::builder()
			// FIXME: These explicit paddings should not be needed.
			.with_attribute(wgpu::VertexFormat::Float32x3)
			.with_offset(16)
			.with_attribute(wgpu::VertexFormat::Float32x3)
			.with_padding_to_size(32)
			.finish(wgpu::VertexStepMode::Vertex)
	}
}

pub fn load_wireframe_shader(
	assets: &mut AssetManager,
	gfx: &GfxContext,
) -> CompRef<wgpu::ShaderModule> {
	assets.cache((), |_| {
		gfx.device
			.create_shader_module(wgpu::ShaderModuleDescriptor {
				label: Some("wireframe.wgsl"),
				source: wgpu::ShaderSource::Wgsl(
					include_str!("../res/shaders/wireframe.wgsl").into(),
				),
			})
	})
}

pub type WireframePipeline = RenderPipeline<(), (WireframeVertex,)>;

pub fn load_wireframe_pipeline(
	assets: &mut AssetManager,
	gfx: &GfxContext,
	surface_format: wgpu::TextureFormat,
	depth_format: wgpu::TextureFormat,
) -> CompRef<WireframePipeline> {
	assets.cache(&surface_format, |assets| {
		let shader = load_wireframe_shader(assets, gfx);

		WireframePipeline::builder()
			.with_vertex_shader(&shader, "vs_entry", &(WireframeVertex::layout(),))
			.with_fragment_shader(&shader, "fs_entry", surface_format)
			.with_vertex_topology(wgpu::PrimitiveTopology::LineList)
			.with_line_draw_mode()
			.with_depth_stencil(wgpu::DepthStencilState {
				format: depth_format,
				depth_write_enabled: false,
				depth_compare: wgpu::CompareFunction::Less,
				bias: Default::default(),
				stencil: Default::default(),
			})
			.finish(&gfx.device)
	})
}
