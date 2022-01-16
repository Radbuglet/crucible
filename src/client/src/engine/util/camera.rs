//! Contains utilities for uploading camera data to the GPU and creating view matrices.

use crate::engine::context::GfxContext;
use crate::engine::input::InputTracker;
use crate::engine::run_loop::RunLoopStatTracker;
use crate::engine::util::std140::Std140;
use crate::engine::util::uniform::UniformManager;
use cgmath::{perspective, Deg, InnerSpace, Matrix3, Matrix4, Rad, Transform, Vector3, Zero};
use crucible_core::util::pod::{align_of_pod, bytes_of_pod, pod_struct, PodWriter};
use crucible_core::util::wrapper::Wrapper;
use std::f32::consts::PI;
use winit::event::VirtualKeyCode;

pod_struct! {
	#[derive(Debug, Copy, Clone)]
	fixed struct CameraUniform {
		proj: Matrix4<f32> [Std140],
	}
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
		let entry = uniform.push(
			align_of_pod::<CameraUniform>(),
			&bytes_of_pod(&CameraUniform { proj: view }),
		);

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
			clipping: (0.1, 500.),
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

pub fn update_camera_free_cam(
	camera: &mut PerspectiveCamera,
	input: &InputTracker,
	run_stats: &RunLoopStatTracker,
) {
	// Calculate heading
	let mut heading = Vector3::zero();

	if input.key(VirtualKeyCode::W).state() {
		heading -= Vector3::unit_z();
	}

	if input.key(VirtualKeyCode::S).state() {
		heading += Vector3::unit_z();
	}

	if input.key(VirtualKeyCode::D).state() {
		heading += Vector3::unit_x();
	}

	if input.key(VirtualKeyCode::A).state() {
		heading -= Vector3::unit_x();
	}

	if input.key(VirtualKeyCode::Q).state() {
		heading -= Vector3::unit_y();
	}

	if input.key(VirtualKeyCode::E).state() {
		heading += Vector3::unit_y();
	}

	let heading = if heading.is_zero() {
		heading
	} else {
		heading.normalize()
	};

	let speed = if input.key(VirtualKeyCode::LShift).state() {
		5.
	} else {
		50.
	};

	// Rotate camera
	let rel = -input.mouse_delta() * 0.3;
	camera.yaw += Deg(rel.x as _).into();
	camera.yaw %= Deg(360.).into();
	camera.pitch += Deg(rel.y as _).into();
	camera.pitch = Rad(camera.pitch.0.clamp(-PI / 2., PI / 2.));

	// Move camera laterally
	let camera_mat = camera.get_world();
	let basis_mat = Matrix3::from_cols(
		camera_mat.x.truncate(),
		camera_mat.y.truncate(),
		camera_mat.z.truncate(),
	);
	camera.position += basis_mat * heading * run_stats.delta_secs() * speed;
}
