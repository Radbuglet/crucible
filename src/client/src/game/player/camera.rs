use std::f32::consts::{PI, TAU};

use crucible_common::voxel::{
	coord::move_rigid_body,
	data::{VoxelChunkData, VoxelWorldData},
	math::EntityVec,
};
use geode::Storage;
use typed_glam::glam::{Mat4, Vec2, Vec3};

#[derive(Debug, Clone, Default)]
pub struct FreeCamController {
	pos: EntityVec,
	pos_vel: EntityVec,
	rot: Vec2,
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct FreeCamInputs {
	pub up: bool,
	pub down: bool,
	pub left: bool,
	pub right: bool,
	pub fore: bool,
	pub back: bool,
}

impl FreeCamInputs {
	pub fn heading(&self) -> Vec3 {
		let mut heading = Vec3::ZERO;

		if self.up {
			heading += Vec3::Y;
		}

		if self.down {
			heading -= Vec3::Y;
		}

		if self.left {
			heading -= Vec3::X;
		}

		if self.right {
			heading += Vec3::X;
		}

		if self.fore {
			heading += Vec3::Z;
		}

		if self.back {
			heading -= Vec3::Z;
		}

		heading.normalize_or_zero()
	}
}

impl FreeCamController {
	pub fn pos(&self) -> EntityVec {
		self.pos
	}

	pub fn facing(&self) -> Vec3 {
		self.rot_matrix().transform_vector3(Vec3::Z)
	}

	pub fn handle_mouse_move(&mut self, delta: Vec2) {
		self.rot += delta * 0.1f32.to_radians();
		self.rot.x = self.rot.x.rem_euclid(TAU);
		self.rot.y = self.rot.y.clamp(-PI / 2., PI / 2.);
	}

	pub fn process(
		&mut self,
		cx: (&VoxelWorldData, &Storage<VoxelChunkData>),
		actions: FreeCamInputs,
	) {
		// Update velocity
		let heading = self
			.rot_matrix()
			.transform_point3(actions.heading())
			.as_dvec3();

		self.pos_vel += heading * 0.2;
		self.pos_vel *= 0.5;

		// Move body
		let size = EntityVec::ONE * 0.5;

		self.pos = move_rigid_body(cx, self.pos - size / 2., size, self.pos_vel) + size / 2.;
	}

	pub fn rot_matrix(&self) -> Mat4 {
		Mat4::from_rotation_y(self.rot.x) * Mat4::from_rotation_x(self.rot.y)
	}

	pub fn view_matrix(&self) -> Mat4 {
		self.rot_matrix().inverse() * Mat4::from_translation(-self.pos().as_glam().as_vec3())
	}
}
