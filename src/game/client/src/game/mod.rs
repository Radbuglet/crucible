pub mod base;
pub mod content;

pub fn register(bhv: &mut bort::BehaviorRegistry) {
	bhv.register_many(content::register)
		.register_many(base::register);
}
