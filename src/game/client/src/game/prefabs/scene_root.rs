use std::time::Duration;

use bort::{
	behavior_kind, delegate, derive_behavior_delegate,
	saddle::{behavior, late_borrow, late_borrow_mut, BehaviorToken},
	BehaviorRegistry, Entity, OwnedEntity, VecEventList, VirtualTag,
};
use crucible_foundation_client::{
	engine::{
		assets::AssetManager,
		gfx::{
			atlas::{AtlasTexture, AtlasTextureGfx},
			camera::CameraManager,
			texture::FullScreenTexture,
		},
		io::{gfx::GfxContext, input::InputManager, viewport::Viewport},
	},
	gfx::{
		skybox::pipeline::{load_skybox_pipeline, SkyboxUniforms},
		voxel::{
			mesh::{self, ChunkVoxelMesh, MaterialVisualDescriptor, WorldVoxelMesh},
			pipeline::{load_opaque_block_pipeline, VoxelUniforms},
		},
	},
};
use crucible_foundation_shared::{
	actor::{
		manager::{ActorManager, ActorSpawned},
		spatial::SpatialTracker,
	},
	material::MaterialRegistry,
	math::{Aabb3, BlockFace, ChunkVec, WorldVec, WorldVecExt},
	voxel::{
		collision::{CollisionMeta, MaterialColliderDescriptor},
		data::{Block, BlockVoxelPointer, ChunkVoxelData, WorldVoxelData},
		loader::{self, LoadedChunk, WorldChunkFactory, WorldLoader},
	},
};
use crucible_util::{debug::error::ResultExt, mem::c_enum::CEnum};
use typed_glam::glam::{UVec2, Vec4};
use wgpu::util::DeviceExt;
use winit::{
	event::{MouseButton, VirtualKeyCode},
	window::CursorGrabMode,
};

use crate::{
	entry::{SceneInitBehavior, SceneRenderHandler, SceneUpdateHandler},
	game::{components::scene_root::GameSceneRoot, prefabs::player::make_local_player},
};

// === Delegates === //

delegate! {
	pub fn ActorSpawnedInGameBehavior(
		bhv: &BehaviorRegistry,
		events: &mut VecEventList<ActorSpawned>,
		engine: Entity,
	)
	as deriving behavior_kind
	as deriving derive_behavior_delegate { event }
}

delegate! {
	pub fn CameraProviderBehavior(
		bhv: &BehaviorRegistry,
		bhv_cx: &mut dyn BehaviorToken<CameraProviderBehavior>,
		actor_tag: VirtualTag,
		mgr: &mut CameraManager
	)
	as deriving behavior_kind
	as deriving derive_behavior_delegate { query }
}

delegate! {
	pub fn ActorInputBehavior(
		bhv: &BehaviorRegistry,
		bhv_cx: &mut dyn BehaviorToken<ActorInputBehavior>,
		scene: Entity,
		actor_tag: VirtualTag,
		input: &InputManager,
	)
	as deriving behavior_kind
	as deriving derive_behavior_delegate { query }
}

delegate! {
	pub fn ActorPhysicsResetBehavior(
		bhv: &BehaviorRegistry,
		bhv_cx: &mut dyn BehaviorToken<ActorPhysicsResetBehavior>,
		actor_tag: VirtualTag,
	)
	as deriving behavior_kind
	as deriving derive_behavior_delegate { query }
}

delegate! {
	pub fn ActorPhysicsInfluenceBehavior(
		bhv: &BehaviorRegistry,
		bhv_cx: &mut dyn BehaviorToken<ActorPhysicsInfluenceBehavior>,
		actor_tag: VirtualTag,
	)
	as deriving behavior_kind
	as deriving derive_behavior_delegate { query }
}

delegate! {
	pub fn ActorPhysicsApplyBehavior(
		bhv: &BehaviorRegistry,
		bhv_cx: &mut dyn BehaviorToken<ActorPhysicsApplyBehavior>,
		actor_tag: VirtualTag,
		spatial_mgr: &mut SpatialTracker,
		world: &WorldVoxelData,
		registry: &MaterialRegistry,
	)
	as deriving behavior_kind
	as deriving derive_behavior_delegate { query }
}

// === Behaviors === //

pub fn register(_bhv: &mut BehaviorRegistry) {}

// === Prefabs === //

pub fn make_game_scene_root(
	bhv_cx: &mut dyn BehaviorToken<SceneInitBehavior>,
	engine: Entity,
	viewport: Entity,
) -> OwnedEntity {
	// Create scene root
	let root = OwnedEntity::new()
		.with_debug_label("game scene root")
		.with(GameSceneRoot { engine, viewport })
		// Actor management
		.with(ActorManager::default())
		.with(SpatialTracker::default())
		// Visual management
		.with(AtlasTexture::new(UVec2::new(16, 16), UVec2::new(2, 2)))
		.with(CameraManager::default())
		// Voxel management
		.with(MaterialRegistry::default())
		.with(WorldVoxelData::default())
		.with(WorldVoxelMesh::default())
		.with(WorldLoader::new(WorldChunkFactory::new(|_world, pos| {
			OwnedEntity::new()
				.with_debug_label(format_args!("chunk at {pos:?}"))
				.with(ChunkVoxelData::default().with_default_air_data())
				.with(ChunkVoxelMesh::default())
				.with(LoadedChunk::default())
				.into_obj()
		})))
		// Handlers
		.with(make_scene_update_handler())
		.with(make_scene_render_handler());

	// Initialize the scene
	behavior! {
		as SceneInitBehavior[bhv_cx] do
		(cx: [
			loader::LoaderUpdateCx;
			mut ActorManager,
			mut AtlasTexture,
			mut AssetManager,
			ref BehaviorRegistry,
			ref GfxContext,
			mut LoadedChunk,
			mut MaterialRegistry,
			mut WorldVoxelData,
			mut WorldLoader,
		], _bhv_cx: []) {{
			// Acquire context
			let actor_mgr = &mut *root.get_mut_s::<ActorManager>(cx);
			let atlas = &mut *root.get_mut_s::<AtlasTexture>(cx);
			let material_registry = &mut *root.get_mut_s::<MaterialRegistry>(cx);
			let world_data = &mut *root.get_mut_s::<WorldVoxelData>(cx);
			let world_loader = &mut *root.get_mut_s::<WorldLoader>(cx);

			let bhv = &*engine.get_s::<BehaviorRegistry>(cx);
			let gfx = &*engine.get_s::<GfxContext>(cx);
			let asset_mgr = &mut *engine.get_mut_s::<AssetManager>(cx);

			// Spawn local player
			let mut on_actor_spawn = VecEventList::new();
			actor_mgr.spawn(&mut on_actor_spawn, make_local_player());
			bhv.process::<ActorSpawnedInGameBehavior>((&mut on_actor_spawn, (root.entity(),)));

			// Load core textures
			let mut atlas_gfx = AtlasTextureGfx::new(gfx, atlas, Some("block texture atlas"));
			let stone_tex = atlas.add(
				&image::load_from_memory(include_bytes!("../res/proto_1.png"))
					.unwrap_pretty()
					.into_rgba32f()
			);
			atlas_gfx.update(gfx, atlas);

			let skybox = image::load_from_memory(include_bytes!("../res/skybox.png"))
				.unwrap_pretty()
				.into_rgba8();

			let skybox = gfx.device.create_texture_with_data(
				&gfx.queue,
				&wgpu::TextureDescriptor {
					label: Some("Skybox panorama"),
					size: wgpu::Extent3d {
						width: skybox.width(),
						height: skybox.height(),
						depth_or_array_layers: 1,
					},
					mip_level_count: 1,
					sample_count: 1,
					dimension: wgpu::TextureDimension::D2,
					format: wgpu::TextureFormat::Rgba8Unorm,
					usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
					view_formats: &[],
				},
				&skybox,
			);

			let skybox = skybox.create_view(&wgpu::TextureViewDescriptor::default());

			// Create atlas and voxel uniform services
			root.insert(VoxelUniforms::new(asset_mgr, gfx, &atlas_gfx.view));
			root.insert(SkyboxUniforms::new(asset_mgr, gfx, &skybox));
			root.insert(atlas_gfx);

			// Register core materials
			material_registry.register("crucible:air", OwnedEntity::new()
				.with_debug_label("air material descriptor"));

			let proto_mat = material_registry.register("crucible:prototype", OwnedEntity::new()
				.with_debug_label("prototype material descriptor")
				.with(MaterialColliderDescriptor::Cubic(CollisionMeta::OPAQUE))
				.with(MaterialVisualDescriptor::cubic_simple(stone_tex)));

			// Setup base world state
			world_loader.temp_load_region(cx, world_data, Aabb3::from_corners_max_excl(
				WorldVec::new(-100, -50, -100).chunk(),
				WorldVec::new(100, 50, 100).chunk() + ChunkVec::ONE,
			));

			let mut pointer = BlockVoxelPointer::new(world_data, WorldVec::ZERO);

			for x in -100..=100 {
				for y in -50..0 {
					for z in -100..=100 {
						pointer.set_pos(Some((cx, world_data)), WorldVec::new(x, y, z));
						pointer.set_state_or_warn(cx, world_data, Block::new(proto_mat.id));
					}
				}
			}
		}}
	}

	root
}

fn make_scene_update_handler() -> SceneUpdateHandler {
	SceneUpdateHandler::new(|me, bhv_cx, _main_loop| {
		behavior! {
			as SceneUpdateHandler[bhv_cx] do
			(cx: [;ref ActorManager, ref GameSceneRoot, ref Viewport], _bhv_cx: []) {
				// Acquire self context
				let actor_mgr = late_borrow(|cx| me.get_s::<ActorManager>(cx));
				let spatial_mgr = late_borrow_mut(|cx| me.get_mut_s::<SpatialTracker>(cx));
				let world_data = late_borrow(|cx| me.get_s::<WorldVoxelData>(cx));
				let block_registry = late_borrow(|cx| me.get_s::<MaterialRegistry>(cx));
				let state = late_borrow(|cx| me.get_s::<GameSceneRoot>(cx));

				let actor_tag = actor_mgr.get(cx).tag();

				// Acquire engine context
				let engine = state.get(cx).engine;
				let main_viewport = state.get(cx).viewport;
				let bhv = late_borrow(|cx| engine.get_s::<BehaviorRegistry>(cx));

				// Acquire viewport context
				let viewport_data = late_borrow(|cx| main_viewport.get_s::<Viewport>(cx));
				let input_mgr = late_borrow(|cx| main_viewport.get_s::<InputManager>(cx));
			}
			(cx: [;ref BehaviorRegistry], bhv_cx: [ActorPhysicsResetBehavior]) {
				bhv.get(cx).process::<ActorPhysicsResetBehavior>((bhv_cx.as_dyn_mut(), actor_tag));
			}
			(cx: [;ref GameSceneRoot, ref BehaviorRegistry, ref Viewport, ref InputManager], bhv_cx: [ActorInputBehavior]) {{
				// Acquire context
				let bhv = &*bhv.get(cx);
				let input_mgr = &*input_mgr.get(cx);
				let viewport_data = &*viewport_data.get(cx);

				// Handle mouse lock
				if input_mgr.button(MouseButton::Left).recently_pressed() {
					viewport_data.window().set_cursor_visible(false);

					for mode in [CursorGrabMode::Locked, CursorGrabMode::Confined] {
						if viewport_data.window().set_cursor_grab(mode).is_ok() {
							break;
						}
					}
				}

				if input_mgr.key(VirtualKeyCode::Escape).recently_pressed() {
					let _ = viewport_data.window().set_cursor_grab(CursorGrabMode::None);
					viewport_data.window().set_cursor_visible(true);
				}

				// Process inputs
				bhv.process::<ActorInputBehavior>((
					bhv_cx.as_dyn_mut(),
					me,
					actor_tag,
					input_mgr,
				));
			}}
			(cx: [;ref BehaviorRegistry], bhv_cx: [ActorPhysicsInfluenceBehavior]) {{
				bhv.get(cx).process::<ActorPhysicsInfluenceBehavior>((bhv_cx.as_dyn_mut(), actor_tag));
			}}
			(
				cx: [;ref BehaviorRegistry, mut SpatialTracker, ref WorldVoxelData, ref MaterialRegistry],
				bhv_cx: [ActorPhysicsApplyBehavior]
			) {
				bhv.get(cx).process::<ActorPhysicsApplyBehavior>((
					bhv_cx.as_dyn_mut(),
					actor_tag,
					&mut *spatial_mgr.get(cx),
					&*world_data.get(cx),
					&*block_registry.get(cx),
				));
			}
		}
	})
}

fn make_scene_render_handler() -> SceneRenderHandler {
	SceneRenderHandler::new(|me, bhv_cx, viewport, frame| {
		behavior! {
			as SceneRenderHandler[bhv_cx] do
			(cx: [;ref ActorManager, ref GameSceneRoot, ref Viewport], _bhv_cx: []) {
				// Acquire self context
				let world_data = late_borrow_mut(|cx| me.get_mut_s::<WorldVoxelData>(cx));
				let world_mesh = late_borrow_mut(|cx| me.get_mut_s::<WorldVoxelMesh>(cx));
				let camera_mgr = late_borrow_mut(|cx| me.get_mut_s::<CameraManager>(cx));
				let atlas_texture = late_borrow(|cx| me.get_s::<AtlasTexture>(cx));
				let material_registry = late_borrow(|cx| me.get_s::<MaterialRegistry>(cx));
				let actor_mgr = late_borrow(|cx| me.get_s::<ActorManager>(cx));
				let state = late_borrow(|cx| me.get_s::<GameSceneRoot>(cx));
				let voxel_uniforms = late_borrow_mut(|cx| me.get_mut_s::<VoxelUniforms>(cx));
				let skybox_uniforms = late_borrow_mut(|cx| me.get_mut_s::<SkyboxUniforms>(cx));

				let actor_tag = actor_mgr.get(cx).tag();

				// Acquire engine context
				let engine = state.get(cx).engine;
				let main_viewport = state.get(cx).viewport;

				let asset_mgr = late_borrow_mut(|cx| engine.get_mut_s::<AssetManager>(cx));
				let bhv = late_borrow(|cx| engine.get_s::<BehaviorRegistry>(cx));
				let gfx = late_borrow(|cx| engine.get_s::<GfxContext>(cx));

				// Ensure that we're rendering the correct viewport
				if viewport != main_viewport {
					return;
				}

				// Acquire viewport context
				let viewport_data = late_borrow(|cx| viewport.get_s::<Viewport>(cx));
				let Some(aspect) = viewport_data.get(cx).curr_surface_aspect() else { return };
			}
			(
				cx: [
					mesh::MeshUpdateCx;
					ref GfxContext,
					mut WorldVoxelData,
					mut WorldVoxelMesh,
					mut ChunkVoxelData,
					ref AtlasTexture,
					ref MaterialRegistry,
				],
				_bhv_cx: [],
			) {{
				let gfx = &*gfx.get(cx);
				let world_data = &mut *world_data.get(cx);
				let world_mesh = &mut *world_mesh.get(cx);
				let atlas_texture = &*atlas_texture.get(cx);
				let material_registry = &*material_registry.get(cx);

				// Consume flagged chunks
				for dirty in world_data.flush_dirty(cx) {
					world_mesh.flag_chunk(dirty.entity());

					for neighbor in BlockFace::variants() {
						let Some(neighbor) = world_data.read_chunk(cx, dirty).neighbor(neighbor) else { continue };
						world_mesh.flag_chunk(neighbor.entity());
					}
				}

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
			(cx: [;ref BehaviorRegistry, mut CameraManager], bhv_cx: [CameraProviderBehavior]) {
				let camera_mgr_snap = {
					let bhv = &*bhv.get(cx);
					let camera_mgr = &mut *camera_mgr.get(cx);

					// Determine the active camera
					camera_mgr.unset();
					bhv.process::<CameraProviderBehavior>((
						bhv_cx.as_dyn_mut(),
						actor_tag,
						&mut *camera_mgr,
					));

					camera_mgr.clone()
				};
			}
			(
				cx: [;
					mut AssetManager,
					ref Viewport,
					mut FullScreenTexture,
					ref GfxContext,
					mut SkyboxUniforms,
					mut VoxelUniforms,
					mut WorldVoxelMesh,
				],
				_bhv_cx: [],
			) {{
				let asset_mgr = &mut *asset_mgr.get(cx);
				let gfx = &*gfx.get(cx);
				let viewport_data = &*viewport_data.get(cx);
				let viewport_depth = &mut *viewport.get_mut_s::<FullScreenTexture>(cx);
				let world_mesh = &mut *world_mesh.get(cx);
				let skybox_uniforms = &mut *skybox_uniforms.get(cx);
				let voxel_uniforms = &mut *voxel_uniforms.get(cx);

				// Setup skybox rendering sub-pass
				{
					let i_proj = camera_mgr_snap.get_proj_xform(aspect).inverse();
					let mut i_view = camera_mgr_snap.get_view_xform().inverse();
					i_view.w_axis = Vec4::new(0.0, 0.0, 0.0, i_view.w_axis.w);

					skybox_uniforms.set_camera_matrix(
						gfx,
						i_view * i_proj,
					);
				}
				let skybox_pipeline = load_skybox_pipeline(asset_mgr, gfx, frame.texture.format());

				// Setup world rendering sub-pass
				voxel_uniforms.set_camera_matrix(gfx, camera_mgr_snap.get_camera_xform(aspect));
				let world_mesh_subpass = world_mesh.prepare_chunk_draw_pass();
				let voxel_pipeline = load_opaque_block_pipeline(
					asset_mgr,
					gfx,
					frame.texture.format(),
					viewport_depth.format(),
				);

				// Begin rendering
				let frame_view = frame
					.texture
					.create_view(&wgpu::TextureViewDescriptor::default());

				let mut cb = gfx
					.device
					.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

				// Render skybox
				{
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
						depth_stencil_attachment: None,
					});

					skybox_pipeline.bind_pipeline(&mut pass);
					skybox_uniforms.write_pass_state(&mut pass);
					pass.draw(0..6, 0..1);
				}

				// Render voxels
				{
					let mut pass = cb.begin_render_pass(&wgpu::RenderPassDescriptor {
						label: None,
						color_attachments: &[Some(wgpu::RenderPassColorAttachment {
							view: &frame_view,
							resolve_target: None,
							ops: wgpu::Operations {
								load: wgpu::LoadOp::Load,
								store: true,
							},
						})],
						depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
							view: viewport_depth.acquire_view(gfx, viewport_data),
							depth_ops: Some(wgpu::Operations {
								load: wgpu::LoadOp::Clear(1.0),
								store: true,
							}),
							stencil_ops: None,
						}),
					});

					voxel_pipeline.bind_pipeline(&mut pass);
					voxel_uniforms.write_pass_state(&mut pass);
					world_mesh_subpass.push(voxel_uniforms, &mut pass);
				}

				// Finish rendering

				gfx.queue.submit([cb.finish()]);
			}}
		}
	})
}
