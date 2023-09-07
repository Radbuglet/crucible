pub mod actor_data;
pub mod actor_rendering;
pub mod core_rendering;
pub mod entry;
pub mod kinematic;
pub mod voxel_data;
pub mod voxel_rendering;

pub fn register(bhv: &mut bort::BehaviorRegistry) {
	bhv.register_many(actor_data::register)
		.register_many(actor_rendering::register)
		.register_many(entry::register)
		.register_many(core_rendering::register)
		.register_many(kinematic::register)
		.register_many(voxel_data::register)
		.register_many(voxel_rendering::register);
}
