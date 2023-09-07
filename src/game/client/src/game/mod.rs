pub mod actors;
pub mod systems;

pub fn register(bhv: &mut bort::BehaviorRegistry) {
	bhv.register_many(actors::register)
		.register_many(systems::register);
}
