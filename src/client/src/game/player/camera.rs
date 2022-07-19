use typed_glam::glam::{Mat4, Vec2, Vec3};

#[derive(Debug, Clone, Default)]
pub struct FreeCamController {
	pos: Vec3,
	pos_vel: Vec3,
	rot: Vec2,
	// rot_vel: Vec2,
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
	pub fn process(&mut self, actions: InputActions) {
		let heading = self.rot_matrix().transform_point3(actions.heading());

		self.pos_vel += heading;
		self.pos_vel *= 0.3;
		self.pos += self.pos_vel;
	}

	pub fn rot_matrix(&self) -> Mat4 {
		Mat4::from_rotation_x(self.rot.y) * Mat4::from_rotation_y(self.rot.x)
	}

	pub fn view_matrix(&self) -> Mat4 {
		Mat4::from_translation(-self.pos) * self.rot_matrix().inverse()
	}
}
