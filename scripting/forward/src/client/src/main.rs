use cgmath::{Angle, Deg, Rad, Vector3, Zero};
use lyptic_core::forward;

pub struct Spatial {
	position: Vector3<f32>,
}

impl Default for Spatial {
	fn default() -> Self {
		Self {
			position: Vector3::zero(),
		}
	}
}

#[forward]
pub trait SpatialT {
	fn position(self) -> Vector3<f32>;
}

impl<'a> SpatialT for &'a Spatial {
	fn position(self) -> Vector3<f32> {
		self.position
	}
}

pub struct ZombieController {
	yaw: Rad<f32>,
}

impl Default for ZombieController {
	fn default() -> Self {
		Self {
			yaw: Deg(30.).into(),
		}
	}
}

pub struct ZombieControllerDeps<'a> {
	obj: &'a ZombieController,
	spatial: &'a mut Spatial,
}

#[forward]
pub trait ZombieControllerT {
	fn lunge(self, dist: f32);
}

impl<'a> ZombieControllerT for ZombieControllerDeps<'a> {
	fn lunge(self, dist: f32) {
		let obj = self.obj;
		self.spatial.position += Vector3::new(obj.yaw.cos(), 0., obj.yaw.sin()) * dist;
		println!("*zombie noises*");
	}
}

#[derive(Default)]
pub struct MyZombie {
	spatial: Spatial,
	controller: ZombieController,
}

impl<'a> SpatialTF for &'a MyZombie {
	type Target = &'a Spatial;

	fn target(self) -> Self::Target {
		&self.spatial
	}
}

impl<'a> ZombieControllerTF for &'a mut MyZombie {
	type Target = ZombieControllerDeps<'a>;

	fn target(self) -> Self::Target {
		ZombieControllerDeps {
			obj: &self.controller,
			spatial: &mut self.spatial,
		}
	}
}

fn main() {
	let mut zombie = MyZombie::default();
	println!("Zombie started out at: {:?}", zombie.position());
	zombie.lunge(3.);
	println!("Zombie is now at: {:?}", zombie.position());
}
