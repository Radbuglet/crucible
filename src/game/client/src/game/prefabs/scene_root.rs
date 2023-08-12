use std::time::Duration;

use bort::{
	auto_reborrow, delegate, derive_behavior_delegate, derive_event_handler,
	derive_multiplexed_handler, BehaviorRegistry, Entity, HasBehavior, OwnedEntity, VecEventList,
	VirtualTag,
};
use crucible_foundation_client::{
	engine::{
		gfx::{atlas::AtlasTexture, camera::CameraManager, texture::FullScreenTexture},
		io::{gfx::GfxContext, input::InputManager},
		scene::{SceneRenderHandler, SceneUpdateHandler},
	},
	gfx::voxel::mesh::{ChunkVoxelMesh, WorldVoxelMesh},
};
use crucible_foundation_shared::{
	actor::{
		manager::{ActorManager, ActorSpawned},
		spatial::SpatialTracker,
	},
	material::MaterialRegistry,
	math::{Aabb3, ChunkVec, EntityVec},
	voxel::{
		data::{ChunkVoxelData, WorldVoxelData},
		loader::{LoadedChunk, WorldChunkFactory, WorldLoader},
	},
};
use typed_glam::glam::UVec2;
use winit::event::VirtualKeyCode;

use crate::game::components::scene_root::GameSceneRoot;

use super::player::make_local_player;

// === Delegates === //

delegate! {
	pub fn ActorSpawnedInGameBehavior(bhv: &BehaviorRegistry, events: &mut VecEventList<ActorSpawned>, engine: Entity)
	as deriving derive_behavior_delegate
	as deriving derive_event_handler
}

impl HasBehavior for ActorSpawnedInGameBehavior {
	type Delegate = Self;
}

delegate! {
	pub fn CameraProviderBehavior(bhv: &BehaviorRegistry, actor_namespace: VirtualTag, mgr: &mut CameraManager)
	as deriving derive_behavior_delegate
	as deriving derive_multiplexed_handler
}

impl HasBehavior for CameraProviderBehavior {
	type Delegate = Self;
}

// === Prefabs === //

pub fn make_game_scene_root(engine: Entity, viewport: Entity) -> OwnedEntity {
	// Create scene root
	let root = OwnedEntity::new()
		.with_debug_label("game scene root")
		.with(GameSceneRoot { engine, viewport })
		.with(ActorManager::default())
		.with(SpatialTracker::default())
		.with(CameraManager::default())
		.with(WorldVoxelData::default())
		.with(WorldVoxelMesh::default())
		.with(WorldLoader::new(WorldChunkFactory::new(|_world, pos| {
			OwnedEntity::new()
				.with_debug_label(format_args!("chunk at {pos:?}"))
				.with(ChunkVoxelData::default())
				.with(ChunkVoxelMesh::default())
				.with(LoadedChunk::default())
				.into_obj()
		})))
		.with(MaterialRegistry::default())
		.with(AtlasTexture::new(UVec2::new(16, 16), UVec2::new(2, 2)))
		.with(SceneUpdateHandler::new(|me, main_loop| {
			let state = &mut *me.get_mut::<GameSceneRoot>();
			let input_mgr = &*state.viewport.get::<InputManager>();

			// Handle quit-on-escape
			if input_mgr.key(VirtualKeyCode::Escape).recently_pressed() {
				main_loop.exit();
			}
		}))
		.with(SceneRenderHandler::new(|me, viewport, frame| {
			// Acquire context
			let mut cx = auto_reborrow! {
				let state = me.get_mut::<GameSceneRoot>();
				let actor_mgr = me.get_mut::<ActorManager>();
				let camera_mgr = me.get_mut::<CameraManager>();
				let world_data = me.get_mut::<WorldVoxelData>();
				let world_mesh = me.get_mut::<WorldVoxelMesh>();
				let material_registry = me.get_mut::<MaterialRegistry>();
				let atlas_texture = me.get_mut::<AtlasTexture>();

				let bhv(state) = state.engine.get::<BehaviorRegistry>();
				let gfx(state) = state.engine.get::<GfxContext>();
				let viewport_depth = viewport.get_mut::<FullScreenTexture>();
			};

			// Determine the active camera
			auto_reborrow!(cx: bhv, actor_mgr, camera_mgr => {
				camera_mgr.unset();
				bhv.process::<CameraProviderBehavior>((actor_mgr.tag(), &mut **camera_mgr));
			});

			// Update the world
			auto_reborrow!(cx: gfx, world_data, world_mesh, atlas_texture, material_registry => {
				world_mesh.update_chunks(
					world_data,
					gfx,
					&atlas_texture,
					material_registry,
					Some(Duration::from_millis(16)),
				);
			});

			// Render a black screen
			auto_reborrow!(cx: gfx, viewport_depth => {
				let frame_view = frame
				.texture
				.create_view(&wgpu::TextureViewDescriptor::default());

				let mut cb = gfx
					.device
					.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

				let mut pass = cb.begin_render_pass(&wgpu::RenderPassDescriptor {
					label: None,
					color_attachments: &[Some(wgpu::RenderPassColorAttachment {
						view: &frame_view,
						resolve_target: None,
						ops: wgpu::Operations {
							load: wgpu::LoadOp::Clear(wgpu::Color {
								r: 0.1,
								g: 0.1,
								b: 0.1,
								a: 1.0,
							}),
							store: true,
						},
					})],
					depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
						view: viewport_depth.acquire_view(&gfx, &*viewport.get()),
						depth_ops: Some(wgpu::Operations {
							load: wgpu::LoadOp::Clear(1.0),
							store: true,
						}),
						stencil_ops: None,
					}),
				});

				// Finish rendering
				drop(pass);
				gfx.queue.submit([cb.finish()]);
			});
		}));

	// Initialize world
	let (root, root_ref) = root.split_guard();
	let mut cx = auto_reborrow! {
		let bhv = engine.get::<BehaviorRegistry>();
		let actor_mgr = root_ref.get_mut::<ActorManager>();
		let material_registry = root_ref.get_mut::<MaterialRegistry>();
		let world_data = root_ref.get_mut::<WorldVoxelData>();
		let world_loader = root_ref.get_mut::<WorldLoader>();
	};

	// Spawn local player
	auto_reborrow!(cx: bhv, actor_mgr => {
		let mut on_actor_spawn = VecEventList::new();
		actor_mgr.spawn(&mut on_actor_spawn, make_local_player());

		bhv.process::<ActorSpawnedInGameBehavior>((&mut on_actor_spawn, (root.entity(),)));
	});

	// Define materials
	auto_reborrow!(cx: material_registry);

	let _air_material = material_registry.register(
		"crucible:air",
		OwnedEntity::new().with_debug_label("air block descriptor"),
	);

	let stone_material = material_registry.register(
		"crucible:stone",
		OwnedEntity::new().with_debug_label("stone block descriptor"),
	);

	// Construct starting island
	auto_reborrow!(cx: world_data, world_loader);

	world_loader.load_region(
		&mut *world_data,
		Aabb3::from_corners(ChunkVec::ZERO, ChunkVec::X),
	);

	root
}

pub fn register(_bhv: &mut BehaviorRegistry) {}
