//! Contains utilities for uploading camera data to the GPU and creating view matrices.

use crate::engine::context::GfxContext;
use crate::engine::util::uniform::UniformManager;
use crate::util::pod_ext::Mat4PodAdapter;
use bytemuck::{bytes_of, Pod, Zeroable};
use cgmath::{perspective, Deg, Matrix4, Rad, Transform, Vector3, Zero};

#[derive(Debug, Pod, Zeroable, Copy, Clone)]
#[repr(C)]
struct CameraUniform {
	proj: Mat4PodAdapter<f32>,
}

#[derive(Debug)]
pub struct GfxCameraManager {
	layout: wgpu::BindGroupLayout,
}

impl GfxCameraManager {
	pub fn new(gfx: &GfxContext) -> Self {
		Self {
			layout: gfx
				.device
				.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
					label: Some("GfxCameraManager bind group"),
					entries: &[
						// uniform CameraUniform
						wgpu::BindGroupLayoutEntry {
							ty: wgpu::BindingType::Buffer {
								ty: wgpu::BufferBindingType::Uniform,
								has_dynamic_offset: false,
								min_binding_size: None,
							},
							binding: 0,
							count: None,
							// Fragment might need it for local-to-world space conversion.
							visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
						},
					],
				}),
		}
	}

	pub fn layout(&self) -> &wgpu::BindGroupLayout {
		&self.layout
	}

	pub fn upload_view(
		&self,
		gfx: &GfxContext,
		uniform: &mut UniformManager,
		view: Matrix4<f32>,
	) -> wgpu::BindGroup {
		let entry = uniform.push(bytes_of(&CameraUniform {
			proj: Mat4PodAdapter(view),
		}));

		gfx.device.create_bind_group(&wgpu::BindGroupDescriptor {
			label: Some("camera uniform group"),
			layout: self.layout(),
			entries: &[wgpu::BindGroupEntry {
				binding: 0,
				resource: wgpu::BindingResource::Buffer(entry),
			}],
		})
	}
}

#[derive(Debug, Clone)]
pub struct PerspectiveCamera {
	// TODO: Generalize spatial objects and Angle3Ds
	pub position: Vector3<f32>,
	pub pitch: Rad<f32>,
	pub yaw: Rad<f32>,
	pub fov: Rad<f32>,
	pub clipping: (f32, f32),
}

impl Default for PerspectiveCamera {
	fn default() -> Self {
		Self {
			position: Vector3::zero(),
			pitch: Rad(0.),
			yaw: Rad(0.),
			fov: Deg(90.).into(),
			clipping: (0.1, 200.),
		}
	}
}

impl PerspectiveCamera {
	pub fn get_world(&self) -> Matrix4<f32> {
		Matrix4::from_translation(self.position)
			* Matrix4::from_angle_y(self.yaw)
			* Matrix4::from_angle_x(self.pitch)
	}

	pub fn get_view_matrix(&self, aspect: f32) -> Matrix4<f32> {
		let world = self.get_world();

		let (near, far) = self.clipping;
		let proj = perspective(self.fov, aspect, near, far);

		proj * world.inverse_transform().unwrap()
	}
}
