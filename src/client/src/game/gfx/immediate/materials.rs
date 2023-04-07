mod opaque_pipeline {
	use crevice::std430::AsStd430;
	use typed_glam::glam;
	use typed_wgpu::vertex::VertexBufferLayout;

	#[derive(Debug, AsStd430)]
	pub struct InputInstance {
		pub pos: glam::Vec3,
		pub color: glam::Vec4,
	}

	impl InputInstance {
		pub fn layout() -> VertexBufferLayout<Self> {
			VertexBufferLayout::builder()
				.with_attribute(wgpu::VertexFormat::Float32x3)
				.with_attribute(wgpu::VertexFormat::Float32x4)
				.finish(wgpu::VertexStepMode::Instance)
		}
	}
}
