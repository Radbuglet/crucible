use geode::ecs_next::world::World;

fn main() {
	let mut world = World::new();
	let mut world_queue = world.queue_ref();

	let friend = world_queue.spawn_deferred();
	assert!(!world_queue.is_alive(friend));
	assert!(world_queue.is_future_entity(friend));
	drop(world_queue);
	world.flush();

	assert!(world.is_alive(friend));
	assert!(!world.is_future_entity(friend));
}
