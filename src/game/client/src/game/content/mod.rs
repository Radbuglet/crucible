pub mod behaviors;
pub mod blocks;
pub mod player;

pub fn register(bhv: &mut bort::BehaviorRegistry) {
	bhv.register_many(blocks::register)
		.register_many(player::register);
}
