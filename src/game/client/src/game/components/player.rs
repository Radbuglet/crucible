use bort::HasGlobalManagedTag;
use crucible_foundation_shared::math::Angle3D;

#[derive(Debug)]
pub struct LocalPlayer {
	pub facing: Angle3D,
}

impl HasGlobalManagedTag for LocalPlayer {
	type Component = Self;
}
