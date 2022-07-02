use std::borrow::Cow;

use geode::prelude::*;

use crate::engine::{gfx::GfxContext, viewport::DEFAULT_SURFACE_FORMAT};

pub struct VoxelRenderingPipeline {
	pub opaque_block_pipeline: wgpu::RenderPipeline,
}

impl VoxelRenderingPipeline {
	pub fn new(_s: Session, gfx: &GfxContext) -> Self {
		let opaque_block_module = gfx
			.device
			.create_shader_module(&wgpu::ShaderModuleDescriptor {
				label: Some("opaque_block.wgsl"),
				source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
					"shaders/opaque_block.wgsl"
				))),
			});

		let opaque_block_pipeline =
			gfx.device
				.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
					label: Some("opaque voxel pipeline"),
					layout: None,
					vertex: wgpu::VertexState {
						module: &opaque_block_module,
						entry_point: "vs_main",
						buffers: &[],
					},
					primitive: wgpu::PrimitiveState {
						topology: wgpu::PrimitiveTopology::TriangleList,
						strip_index_format: None,
						front_face: wgpu::FrontFace::Ccw,
						cull_mode: None,
						unclipped_depth: false,
						polygon_mode: wgpu::PolygonMode::Fill,
						conservative: false,
					},
					depth_stencil: None,
					multisample: wgpu::MultisampleState {
						count: 1,
						mask: !0,
						alpha_to_coverage_enabled: false,
					},
					fragment: Some(wgpu::FragmentState {
						module: &opaque_block_module,
						entry_point: "fs_main",
						targets: &[wgpu::ColorTargetState {
							// TODO: Dynamically adapt format
							format: DEFAULT_SURFACE_FORMAT,
							blend: None,
							write_mask: wgpu::ColorWrites::all(),
						}],
					}),
					multiview: None,
				});

		Self {
			opaque_block_pipeline,
		}
	}
}
