use bort::{storage, Entity, OwnedEntity};
use crucible_common::{
	game::actor::{ActorManager, Tag},
	voxel::math::EntityVec,
};
use typed_glam::glam::Vec2;

// === Factory === //

#[non_exhaustive]
pub struct LocalPlayerTag;

impl Tag for LocalPlayerTag {}

pub fn spawn_player(actors: &mut ActorManager) -> Entity {
	actors.spawn(
		LocalPlayerTag::TAG,
		OwnedEntity::new()
			.with_debug_label("local player")
			.with(LocalPlayerState {
				pos: EntityVec::ZERO,
				vel: EntityVec::ZERO,
				rot: Vec2::ZERO,
			}),
	)
}

// === Systems === //

pub fn update_local_players(actors: &ActorManager) {
	let states = storage::<LocalPlayerState>();

	for player in actors.tagged::<LocalPlayerTag>() {
		let player_state = &mut *states.get_mut(player);
		player_state.pos += player_state.vel;
		player_state.vel += EntityVec::NEG_Y;
	}
}

// === Components === //

#[derive(Debug)]
pub struct LocalPlayerState {
	pos: EntityVec,
	vel: EntityVec,
	rot: Vec2,
}
