pub mod extra_blocks;
pub mod player;

pub fn register(bhv: &mut bort::BehaviorRegistry) {
	bhv.register_many(extra_blocks::register)
		.register_many(player::register);
}
