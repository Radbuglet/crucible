use std::time::Duration;

use bort::{
	behavior_kind, delegate, derive_behavior_delegate,
	saddle::{behavior, late_borrow, late_borrow_mut, BehaviorToken},
	BehaviorRegistry, Entity, HasBehavior, OwnedEntity, VecEventList, VirtualTag,
};
use crucible_foundation_client::{
	engine::{
		gfx::{atlas::AtlasTexture, camera::CameraManager, texture::FullScreenTexture},
		io::{gfx::GfxContext, input::InputManager, viewport::Viewport},
	},
	gfx::voxel::mesh::{self, ChunkVoxelMesh, WorldVoxelMesh},
};
use crucible_foundation_shared::{
	actor::{
		manager::{ActorManager, ActorSpawned},
		spatial::SpatialTracker,
	},
	material::MaterialRegistry,
	voxel::{
		data::{ChunkVoxelData, WorldVoxelData},
		loader::{LoadedChunk, WorldChunkFactory, WorldLoader},
	},
};
use typed_glam::glam::UVec2;
use winit::event::VirtualKeyCode;

use crate::{
	entry::{SceneRenderHandler, SceneUpdateHandler},
	game::components::scene_root::GameSceneRoot,
};

// === Delegates === //

delegate! {
	pub fn ActorSpawnedInGameBehavior(bhv: &BehaviorRegistry, events: &mut VecEventList<ActorSpawned>, engine: Entity)
	as deriving derive_behavior_delegate { event }
}

impl HasBehavior for ActorSpawnedInGameBehavior {
	type Delegate = Self;
}

delegate! {
	pub fn CameraProviderDelegate(
		bhv: &BehaviorRegistry,
		bhv_cx: &mut dyn BehaviorToken<CameraProviderDelegate>,
		actor_namespace: VirtualTag,
		mgr: &mut CameraManager
	)
	as deriving derive_behavior_delegate { query }
	as deriving behavior_kind
}

// === Prefabs === //

pub fn make_game_scene_root(engine: Entity, viewport: Entity) -> OwnedEntity {
	// Create scene root
	OwnedEntity::new()
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
		.with(SceneUpdateHandler::new(|me, bhv_cx, main_loop| {
			behavior! {
				as SceneUpdateHandler[bhv_cx] do
				(cx: [;mut GameSceneRoot, ref InputManager], _bhv_cx: []) {{
					let state = me.get_mut_s::<GameSceneRoot>(cx);
					let input_mgr = state.viewport.get_s::<InputManager>(cx);

					// Handle quit-on-escape
					if input_mgr.key(VirtualKeyCode::Escape).recently_pressed() {
						main_loop.exit();
					}
				}}
			}
		}))
		.with(SceneRenderHandler::new(|me, bhv_cx, viewport, frame| {
			behavior! {
				as SceneRenderHandler[bhv_cx] do
				(cx: [;ref GameSceneRoot, ref ActorManager], _bhv_cx: []) {
					// Acquire self context
					let world_data = late_borrow(|cx| me.get_s::<WorldVoxelData>(cx));
					let world_mesh = late_borrow_mut(|cx| me.get_mut_s::<WorldVoxelMesh>(cx));
					let camera_mgr = late_borrow_mut(|cx| me.get_mut_s::<CameraManager>(cx));
					let atlas_texture = late_borrow(|cx| me.get_s::<AtlasTexture>(cx));
					let material_registry = late_borrow(|cx| me.get_s::<MaterialRegistry>(cx));
					let actor_mgr = late_borrow(|cx| me.get_s::<ActorManager>(cx));
					let state = late_borrow(|cx| me.get_s::<GameSceneRoot>(cx));

					let actor_tag = actor_mgr.get(cx).tag();

					// Acquire engine context
					let engine = state.get(cx).engine;
					let main_viewport = state.get(cx).viewport;

					let bhv = late_borrow(|cx| engine.get_s::<BehaviorRegistry>(cx));
					let gfx = late_borrow(|cx| engine.get_s::<GfxContext>(cx));

					// Ensure that we're rendering the correct viewport
					if viewport != main_viewport {
						return;
					}
				}
				(cx: [;ref BehaviorRegistry, mut CameraManager], bhv_cx: [CameraProviderDelegate]) {{
					let bhv = &*bhv.get(cx);
					let camera_mgr = &mut *camera_mgr.get(cx);

					// Determine the active camera
					camera_mgr.unset();
					bhv.process::<CameraProviderDelegate>((
						bhv_cx.as_dyn_mut(),
						actor_tag,
						camera_mgr,
					));
				}}
				(
					cx: [
						mesh::CxMut;
						ref GfxContext,
						ref WorldVoxelData,
						mut WorldVoxelMesh,
						ref AtlasTexture,
						ref MaterialRegistry,
					],
					_bhv_cx: [],
				) {{
					let gfx = &*gfx.get(cx);
					let world_data = &*world_data.get(cx);
					let world_mesh = &mut *world_mesh.get(cx);
					let atlas_texture = &*atlas_texture.get(cx);
					let material_registry = &*material_registry.get(cx);

					// Update the world
					world_mesh.update_chunks(
						cx,
						world_data,
						gfx,
						atlas_texture,
						material_registry,
						Some(Duration::from_millis(16)),
					);
				}}
				(
					cx: [;ref GfxContext, mut FullScreenTexture, ref Viewport],
					_bhv_cx: [],
				) {{
					let gfx = &*gfx.get(cx);
					let viewport_depth = &mut *main_viewport.get_mut_s::<FullScreenTexture>(cx);

					// Render a black screen
					let frame_view = frame
						.texture
						.create_view(&wgpu::TextureViewDescriptor::default());

					let mut cb = gfx
						.device
						.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

					let pass = cb.begin_render_pass(&wgpu::RenderPassDescriptor {
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
							view: viewport_depth.acquire_view(gfx, &main_viewport.get_s(cx)),
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
				}}
			}
		}))
}

pub fn register(_bhv: &mut BehaviorRegistry) {}
