use std::time::Duration;

use bort::{
	alias, call_cx, proc, saddle_delegate, BehaviorRegistry, Entity, OwnedEntity, VecEventList,
	VirtualTag,
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
		ui::{brush::ImmRenderer, materials::sdf_rect::SdfRectImmBrushExt},
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
	math::{Aabb2, Aabb3, BlockFace, ChunkVec, Color4, WorldVec, WorldVecExt},
	voxel::{
		collision::{CollisionMeta, MaterialColliderDescriptor},
		data::{Block, BlockVoxelPointer, ChunkVoxelData, WorldVoxelData},
		loader::{self, LoadedChunk, WorldChunkFactory, WorldLoader},
	},
};
use crucible_util::{debug::error::ResultExt, mem::c_enum::CEnum};
use typed_glam::glam::{UVec2, Vec2, Vec4};
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

saddle_delegate! {
	pub fn ActorSpawnedInGameBehavior(
		events: &mut VecEventList<ActorSpawned>,
		engine: Entity,
	)
}

saddle_delegate! {
	pub fn CameraProviderBehavior(
		actor_tag: VirtualTag,
		mgr: &mut CameraManager
	)
}

saddle_delegate! {
	pub fn ActorInputBehavior(
		scene: Entity,
		actor_tag: VirtualTag,
		input: &InputManager,
	)
}

saddle_delegate! {
	pub fn ActorPhysicsResetBehavior(actor_tag: VirtualTag)
}

saddle_delegate! {
	pub fn ActorPhysicsInfluenceBehavior(actor_tag: VirtualTag)
}

saddle_delegate! {
	pub fn ActorPhysicsApplyBehavior(
		actor_tag: VirtualTag,
		spatial_mgr: &mut SpatialTracker,
		world: &WorldVoxelData,
		registry: &MaterialRegistry,
	)
}

// === Behaviors === //

pub fn register(_bhv: &mut BehaviorRegistry) {}

// === Prefabs === //

alias! {
	let asset_mgr: AssetManager;
	let actor_mgr: ActorManager;
	let atlas_texture: AtlasTexture;
	let bhv: BehaviorRegistry;
	let block_registry: MaterialRegistry;
	let camera_mgr: CameraManager;
	let gfx: GfxContext;
	let input_mgr: InputManager;
	let material_registry: MaterialRegistry;
	let skybox_uniforms: SkyboxUniforms;
	let spatial_mgr: SpatialTracker;
	let state: GameSceneRoot;
	let viewport_data: Viewport;
	let voxel_uniforms: VoxelUniforms;
	let world_data: WorldVoxelData;
	let world_loader: WorldLoader;
	let world_mesh: WorldVoxelMesh;
}

pub fn make_game_scene_root(
	call_cx: &mut call_cx![SceneInitBehavior],
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
	proc! {
		as SceneInitBehavior[call_cx] do
		(
			cx: [mut LoadedChunk; loader::LoaderUpdateCx],
			call_cx: [ActorSpawnedInGameBehavior],
			ref bhv = engine,
			ref gfx = engine,
			mut asset_mgr = engine,
			mut actor_mgr = root,
			mut atlas_texture = root,
			mut material_registry = root,
			mut world_data = root,
			mut world_loader = root,
		) {{
			// Spawn local player
			let mut on_actor_spawn = VecEventList::new();
			actor_mgr.spawn(&mut on_actor_spawn, make_local_player());
			bhv.get::<ActorSpawnedInGameBehavior>()(call_cx, &mut on_actor_spawn, root.entity());

			// Load core textures
			let mut atlas_gfx = AtlasTextureGfx::new(gfx, atlas_texture, Some("block texture atlas"));
			let stone_tex = atlas_texture.add(
				&image::load_from_memory(include_bytes!("../res/proto_1.png"))
					.unwrap_pretty()
					.into_rgba32f()
			);
			atlas_gfx.update(gfx, atlas_texture);

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
	SceneUpdateHandler::new(|bhv, call_cx, me, _main_loop| {
		proc! {
			as SceneUpdateHandler[call_cx] do
			(_cx: [], _call_cx: [], ref state = me, ref actor_mgr = me) {
				let main_viewport = state.viewport;
				let actor_tag = actor_mgr.tag();
			}
			(cx: [ref BehaviorRegistry], call_cx: [ActorPhysicsResetBehavior]) {
				bhv.get::<ActorPhysicsResetBehavior>()(call_cx, actor_tag);
			}
			(cx: [], call_cx: [ActorInputBehavior], ref viewport_data = main_viewport, ref input_mgr = main_viewport) {
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
				bhv.get::<ActorInputBehavior>()(call_cx, me, actor_tag, &input_mgr);
			}
			(_cx: [], call_cx: [ActorPhysicsInfluenceBehavior]) {
				bhv.get::<ActorPhysicsInfluenceBehavior>()(call_cx, actor_tag);
			}
			(_cx: [], call_cx: [ActorPhysicsApplyBehavior], mut spatial_mgr = me, ref world_data = me, ref block_registry = me) {
				bhv.get::<ActorPhysicsApplyBehavior>()(
					call_cx,
					actor_tag,
					spatial_mgr,
					world_data,
					block_registry,
				);
			}
		}
	})
}

fn make_scene_render_handler() -> SceneRenderHandler {
	SceneRenderHandler::new(|bhv, call_cx, me, viewport, frame| {
		proc! {
			as SceneRenderHandler[call_cx] do
			(cx: [ref Viewport], _call_cx: [], ref state = me, ref actor_mgr = me) {
				let engine = state.engine;
				let main_viewport = state.viewport;
				let actor_tag = actor_mgr.tag();

				if viewport != main_viewport {
					return;
				}

				let Some(aspect) = viewport.get_s::<Viewport>(cx).curr_surface_aspect() else { return };
			}
			(
				cx: [mut ChunkVoxelData; mesh::MeshUpdateCx],
				_call_cx: [],
				ref gfx = engine,
				mut world_data = me,
				mut world_mesh = me,
				ref atlas_texture = me,
				ref material_registry = me,
			) {
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
			}
			(cx: [mut CameraManager], call_cx: [CameraProviderBehavior], mut camera_mgr = me) {
				// Determine the active camera
				camera_mgr.unset();
				bhv.get::<CameraProviderBehavior>()(
					call_cx,
					actor_tag,
					camera_mgr,
				);
				let camera_mgr_snap = camera_mgr.clone();
			}
			(
				cx: [
					mut FullScreenTexture,
					ref GfxContext,
					mut SkyboxUniforms,
					mut VoxelUniforms,
					mut WorldVoxelMesh,
				],
				_call_cx: [],
				ref gfx = engine,
				mut asset_mgr = engine,
				ref viewport_data = viewport,
				mut world_mesh = me,
				mut skybox_uniforms = me,
				mut voxel_uniforms = me,
			) {{
				let viewport_depth = &mut *viewport.get_mut_s::<FullScreenTexture>(cx);

				// Setup skybox rendering sub-pass
				{
					let i_proj = camera_mgr_snap.get_proj_xform(aspect).inverse();
					let mut i_view = camera_mgr_snap.get_view_xform().inverse();
					i_view.w_axis = Vec4::new(0.0, 0.0, 0.0, i_view.w_axis.w);

					skybox_uniforms.set_camera_matrix(
						&gfx,
						i_view * i_proj,
					);
				}
				let skybox_pipeline = load_skybox_pipeline(asset_mgr, gfx, frame.texture.format());

				// Setup world rendering sub-pass
				voxel_uniforms.set_camera_matrix(&gfx, camera_mgr_snap.get_camera_xform(aspect));
				let world_mesh_subpass = world_mesh.prepare_chunk_draw_pass();
				let voxel_pipeline = load_opaque_block_pipeline(
					asset_mgr,
					gfx,
					frame.texture.format(),
					viewport_depth.format(),
				);

				// Setup UI rendering sub-pass
				let mut ui = ImmRenderer::new();
				ui.brush().fill_rect(
					Aabb2::from_origin_size(Vec2::ZERO, Vec2::splat(0.05), Vec2::splat(0.5)),
					Color4::new(1.0, 0.0, 0.0, 0.5),
				);

				let ui = ui.prepare_render(
					gfx,
					asset_mgr,
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
							view: viewport_depth.acquire_view(&gfx, &viewport_data),
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

				// Render UI
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
							view: viewport_depth.acquire_view(&gfx, &viewport_data),
							depth_ops: Some(wgpu::Operations {
								load: wgpu::LoadOp::Clear(0.0),
								store: true,
							}),
							stencil_ops: None,
						}),
					});

					ui.render(&mut pass);
				}

				// Finish rendering
				gfx.queue.submit([cb.finish()]);
			}}
		}
	})
}
