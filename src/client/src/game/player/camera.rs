use std::f32::consts::{PI, TAU};

use typed_glam::glam::{Mat4, Vec2, Vec3};

#[derive(Debug, Clone, Default)]
pub struct FreeCamController {
	pos: Vec3,
	pos_vel: Vec3,
	rot: Vec2,
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct InputActions {
	pub up: bool,
	pub down: bool,
	pub left: bool,
	pub right: bool,
	pub fore: bool,
	pub back: bool,
}

impl InputActions {
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
	pub fn pos(&self) -> Vec3 {
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

	pub fn process(&mut self, actions: InputActions) {
		let heading = self.rot_matrix().transform_point3(actions.heading());

		self.pos_vel += heading;
		self.pos_vel *= 0.7;
		self.pos += self.pos_vel * 0.3;
	}

	pub fn rot_matrix(&self) -> Mat4 {
		Mat4::from_rotation_y(self.rot.x) * Mat4::from_rotation_x(self.rot.y)
	}

	pub fn view_matrix(&self) -> Mat4 {
		self.rot_matrix().inverse() * Mat4::from_translation(-self.pos)
	}
}
