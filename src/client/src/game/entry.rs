use crucible_common::voxel::data::{ChunkData, WorldData};
use crucible_core::ecs::{
	context::{unpack, Provider},
	core::{Archetype, Entity, Storage},
	userdata::Userdata,
};

use super::player::camera::FreeCamController;

#[derive(Debug)]
pub struct PlayScene {
	// Archetypes
	arch_world: Archetype,
	arch_player: Archetype,

	// Storages
	world_datas: Storage<WorldData>,
	chunk_datas: Storage<ChunkData>,
	cameras: Storage<FreeCamController>,

	// Resources
	local_player: Entity,
}

impl PlayScene {
	pub fn on_update(cx: &mut impl Provider) {
		unpack!(cx => {
			&me = &Entity,
			userdatas = &mut Storage<Userdata>,
		});

		let me = userdatas.get_downcast_mut::<Self>(me);
	}

	pub fn on_render() {}
}
