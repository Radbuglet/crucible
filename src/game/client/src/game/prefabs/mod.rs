pub mod player;
pub mod scene_root;
pub mod spatial_bundle;

pub fn register(bhv: &mut bort::BehaviorRegistry) {
	bhv.register_many(scene_root::register)
		.register_many(spatial_bundle::register)
		.register_many(player::register);
}
