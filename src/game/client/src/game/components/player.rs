use bort::HasGlobalManagedTag;
use crucible_foundation_shared::math::Angle3D;

#[derive(Debug)]
pub struct LocalPlayer {
	pub facing: Angle3D,
	pub fly_mode: bool,
	pub jump_cool_down: u64,
}

impl HasGlobalManagedTag for LocalPlayer {
	type Component = Self;
}
