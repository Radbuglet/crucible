use bort::CompRef;
use crevice::std430::AsStd430;
use crucible_foundation_shared::math::{compose_f32, Aabb2, Color4, Sign, ZERO_TO_ONE_EXPONENT};
use typed_glam::glam::{Affine2, Vec2, Vec4};
use typed_wgpu::{
	buffer::BufferSlice,
	pipeline::RenderPipeline,
	vertex::{Std430VertexFormat, VertexBufferLayout},
};
use wgpu::util::{BufferInitDescriptor, DeviceExt};

use crate::{
	engine::{assets::AssetManager, io::gfx::GfxContext},
	gfx::ui::brush::{ImmBrush, ImmMaterial, ImmPass},
};

// === Pipeline === //

#[derive(Debug, AsStd430)]
struct Instance {
	linear_x: Vec2,
	linear_y: Vec2,
	translation: Vec2,
	depth: f32,
	color: Vec4,
}

impl Instance {
	pub fn layout() -> VertexBufferLayout<Self> {
		VertexBufferLayout::builder()
			.with_attribute(Std430VertexFormat::Float32x2) // linear_x
			.with_attribute(Std430VertexFormat::Float32x2) // linear_y
			.with_attribute(Std430VertexFormat::Float32x2) // translation
			.with_attribute(Std430VertexFormat::Float32) // depth
			.with_attribute(Std430VertexFormat::Float32x4) // color
			.finish(wgpu::VertexStepMode::Instance)
	}
}

fn load_shader(
	gfx: &GfxContext,
	assets: &mut AssetManager,
) -> CompRef<'static, wgpu::ShaderModule> {
	assets.cache((), |_| {
		gfx.device
			.create_shader_module(wgpu::ShaderModuleDescriptor {
				label: Some("SDF rect shader"),
				source: wgpu::ShaderSource::Wgsl(include_str!("sdf_rect.wgsl").into()),
			})
	})
}

type SdfRectPipeline = RenderPipeline<(), (Instance,)>;

fn load_pipeline(
	gfx: &GfxContext,
	assets: &mut AssetManager,
	surface_format: wgpu::TextureFormat,
	depth_format: wgpu::TextureFormat,
) -> CompRef<'static, SdfRectPipeline> {
	assets.cache(&(surface_format, depth_format), |assets| {
		let shader = load_shader(gfx, assets);

		SdfRectPipeline::builder()
			.with_vertex_shader(&shader, "vs_main", &(Instance::layout(),))
			.with_fragment_shader_alpha_blend(&shader, "fs_main", surface_format)
			.with_depth(depth_format, true, wgpu::CompareFunction::Greater)
			.finish(&gfx.device)
	})
}

// === Material === //

#[derive(Debug, Default)]
struct Material {
	instances: Vec<u8>,
	layer_starts: Vec<usize>,
}

impl ImmMaterial for Material {
	type Config = ();
	type Pass<'a> = Pass<'a>;

	fn new((): &mut Self::Config) -> Self {
		Self::default()
	}

	fn create_layer(&mut self) {
		self.layer_starts.push(self.instances.len());
	}

	fn create_pass<'a>(
		&'a mut self,
		gfx: &'a GfxContext,
		assets: &mut AssetManager,
		surface_format: wgpu::TextureFormat,
		depth_format: wgpu::TextureFormat,
	) -> Self::Pass<'a> {
		let pipeline = load_pipeline(gfx, assets, surface_format, depth_format);
		let all_instances = gfx.device.create_buffer_init(&BufferInitDescriptor {
			label: Some("SDF rectangle instances"),
			usage: wgpu::BufferUsages::VERTEX,
			contents: &self.instances,
		});

		Pass {
			material: self,
			all_instances,
			pipeline,
		}
	}
}

#[derive(Debug)]
struct Pass<'a> {
	material: &'a Material,
	pipeline: CompRef<'static, SdfRectPipeline>,
	all_instances: wgpu::Buffer,
}

impl ImmPass for Pass<'_> {
	fn render<'a>(&'a self, layer: usize, pass: &mut wgpu::RenderPass<'a>) {
		let from_byte = self.material.layer_starts[layer];
		let to_byte_excl = self
			.material
			.layer_starts
			.get(layer + 1)
			.copied()
			.unwrap_or(self.material.instances.len());

		let instance_count = ((to_byte_excl - from_byte) / Instance::std430_size_static()) as u32;

		let byte_range = (from_byte as wgpu::BufferAddress)..(to_byte_excl as wgpu::BufferAddress);

		// Bind state
		self.pipeline.bind_pipeline(pass);
		SdfRectPipeline::bind_vertex_buffer(
			pass,
			BufferSlice::wrap(self.all_instances.slice(byte_range)),
		);
		pass.draw(0..6, 0..instance_count);
		log::info!("Rendering {instance_count} rectangle(s) (range: {from_byte}..{to_byte_excl})!");
	}
}

// === Extension === //

pub trait SdfRectImmBrushExt {
	fn fill_rect(self, aabb: Aabb2<Vec2>, color: Color4);
}

impl SdfRectImmBrushExt for ImmBrush<'_> {
	fn fill_rect(self, aabb: Aabb2<Vec2>, color: Color4) {
		let inner = &mut *self.renderer.use_inner();
		let depth = inner.alloc_depth();
		let depth = compose_f32(Sign::Positive, ZERO_TO_ONE_EXPONENT, depth as u32);

		// The base mesh is from (-1, -1) to (1, 1). We need to transform that into our AABB.
		let transform =
			// Apply the brush's transformation to our rectangle
			self.transform *
			// `(x, y), (x + w, y + h)`
			Affine2::from_translation(aabb.origin) *
			// from `(0, 0), (1, 1)` to `(0, 0), (w, h)`
			Affine2::from_scale(aabb.size) *
			// from `(-1, -1), (1, 1)` to `(0, 0), (1, 1)`.
			Affine2::from_scale(Vec2::splat(0.5)) * Affine2::from_translation(Vec2::splat(1.0));

		inner
			.find_and_get_material::<Material>(())
			.instances
			.extend(
				Instance {
					linear_x: transform.x_axis,
					linear_y: transform.y_axis,
					translation: transform.translation,
					depth,
					color: color.to_glam(),
				}
				.as_std430()
				.as_bytes(),
			);
	}
}
