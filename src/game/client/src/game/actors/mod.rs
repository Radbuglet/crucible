pub mod player;

pub fn register(bhv: &mut bort::BehaviorRegistry) {
	bhv.register_many(player::register);
}
