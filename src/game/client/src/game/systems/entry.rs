use std::{f32::consts::PI, time::Duration};

use bort::{
	alias, call_cx, delegate, proc, proc_collection, saddle_delegate, BehaviorProvider,
	BehaviorRegistry, Entity, OwnedEntity, VecEventList, VirtualTag,
};
use crucible_foundation_client::{
	engine::{
		assets::AssetManager,
		gfx::{atlas::AtlasTexture, camera::CameraManager, texture::FullScreenTexture},
		io::{gfx::GfxContext, input::InputManager, viewport::Viewport},
	},
	gfx::{
		actor::{
			mesh::ActorRenderer,
			pipeline::{load_opaque_actor_pipeline, ActorRenderingUniforms},
		},
		skybox::pipeline::{load_skybox_pipeline, SkyboxUniforms},
		ui::{brush::ImmRenderer, materials::sdf_rect::SdfRectImmBrushExt},
		voxel::{
			mesh::{MeshUpdateCx, WorldVoxelMesh},
			pipeline::{load_opaque_block_pipeline, VoxelUniforms},
		},
	},
};
use crucible_foundation_shared::{
	actor::{
		manager::{ActorManager, ActorSpawned},
		spatial::SpatialTracker,
	},
	bort::lifecycle::{LifecycleManager, PartialEntity},
	material::MaterialRegistry,
	math::{Aabb2, Aabb3, BlockFace, ChunkVec, Color3, Color4, WorldVec, WorldVecExt},
	voxel::{
		data::{Block, BlockVoxelPointer, ChunkVoxelData, WorldVoxelData},
		loader::{LoaderUpdateCx, WorldLoader},
		mesh::QuadMeshLayer,
	},
};
use crucible_util::mem::c_enum::CEnum;
use typed_glam::glam::{Affine3A, Vec2, Vec3, Vec4};
use winit::{
	event::{MouseButton, VirtualKeyCode},
	window::CursorGrabMode,
};

use crate::{
	entry::{SceneInitBehavior, SceneRenderHandler, SceneUpdateHandler},
	game::actors::player::spawn_local_player,
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

// === GameInitManager === //

delegate! {
	pub fn GameSceneInitBehavior(
		bhv: BehaviorProvider<'_>,
		call_cx: &mut call_cx![GameSceneInitBehavior],
		scene: PartialEntity<'_>,
		engine: Entity,
	)
	as deriving proc_collection
}

pub type GameInitRegistry = LifecycleManager<GameSceneInitBehavior>;

// === Aliases === //

alias! {
	let asset_mgr: AssetManager;
	let actor_mgr: ActorManager;
	let actor_uniforms: ActorRenderingUniforms;
	let actor_renderer: ActorRenderer;
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
	let viewport_depth: FullScreenTexture;
	let voxel_uniforms: VoxelUniforms;
	let world_data: WorldVoxelData;
	let world_loader: WorldLoader;
	let world_mesh: WorldVoxelMesh;
}

// === Behaviors === //

pub fn register(bhv: &mut BehaviorRegistry) {
	let _ = bhv;
}

// === Components === //

#[derive(Debug)]
pub struct GameSceneRoot {
	pub engine: Entity,
	pub viewport: Entity,
}

// === Prefabs === //

pub fn spawn_game_scene_root(
	call_cx: &mut call_cx![SceneInitBehavior],
	engine: Entity,
	viewport: Entity,
) -> OwnedEntity {
	// Spawn base entity
	let root = OwnedEntity::new()
		.with_debug_label("game scene root")
		.with(GameSceneRoot { engine, viewport })
		.with(make_scene_update_handler())
		.with(make_scene_render_handler());

	// Run external initializers
	let pm = GameInitRegistry::new()
		.with_many(super::actor_data::push_plugins)
		.with_many(super::actor_rendering::push_plugins)
		.with_many(super::core_rendering::push_plugins)
		.with_many(super::voxel_data::push_plugins)
		.with_many(super::voxel_rendering::push_plugins);

	proc! {
		as SceneInitBehavior[call_cx] do
		(_cx: [], call_cx: [GameSceneInitBehavior], ref bhv = engine) {
			pm.execute(|delegate, scene| delegate(bhv.provider(), call_cx, scene, engine), root.entity());
		}
	}

	// Setup initial scene
	proc! {
		as SceneInitBehavior[call_cx] do
		(
			cx: [; LoaderUpdateCx],
			call_cx: [ActorSpawnedInGameBehavior],
			mut actor_mgr = root,
			ref bhv = engine,
			mut world_loader = root,
			mut world_data = root,
			ref material_registry = root,
		) {
			// Populate initial world data
			world_loader.temp_load_region(
				cx,
				world_data,
				Aabb3::from_corners_max_excl(
					WorldVec::new(-100, -50, -100).chunk(),
					WorldVec::new(100, 50, 100).chunk() + ChunkVec::ONE,
				),
			);

			let mut pointer = BlockVoxelPointer::new(&world_data, WorldVec::ZERO);
			let proto_mat = material_registry.find_by_name("crucible:proto").unwrap();

			for x in -100..=100 {
				for y in -50..0 {
					for z in -100..=100 {
						pointer.set_pos(Some((cx, world_data)), WorldVec::new(x, y, z));
						pointer.set_state_or_warn(cx, world_data, Block::new(proto_mat.id));
					}
				}
			}

			// Create player
			let mut on_spawned = VecEventList::new();
			actor_mgr.spawn(&mut on_spawned, spawn_local_player());
			bhv.get::<ActorSpawnedInGameBehavior>()(call_cx, &mut on_spawned, root.entity());
		}
	}

	root
}

// === Handlers === //

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

				let Some(viewport_size) = viewport.get_s::<Viewport>(cx).curr_surface_size() else { return };
				let viewport_size = viewport_size.as_vec2();
				let Some(aspect) = viewport.get_s::<Viewport>(cx).curr_surface_aspect() else { return };
			}
			(
				cx: [mut ChunkVoxelData; MeshUpdateCx],
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
				mut actor_renderer = me,
				ref actor_uniforms = me,
				mut asset_mgr = engine,
				ref viewport_data = viewport,
				mut viewport_depth = viewport,
				mut world_mesh = me,
				mut skybox_uniforms = me,
				mut voxel_uniforms = me,
			) {
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

				// Setup actor rendering
				actor_uniforms.set_camera_matrix(gfx, camera_mgr_snap.get_camera_xform(aspect));
				let actor_pipeline = load_opaque_actor_pipeline(
					asset_mgr,
					gfx,
					frame.texture.format(),
					viewport_depth.format(),
				);

				actor_renderer.push_model(gfx, &QuadMeshLayer::new()
					.with_cube(
						Aabb3::from_origin_size(Vec3::X * -0.3, Vec3::new(0.45, 0.95, 0.45), Vec3::new(0.5, 0.0, 0.5)),
						Color3::new(0.5, 0.5, 0.5)
					)
					.with_cube(
						Aabb3::from_origin_size(Vec3::X * 0.3, Vec3::new(0.45, 0.95, 0.45), Vec3::new(0.5, 0.0, 0.5)),
						Color3::new(0.5, 0.5, 0.5)
					)
					.with_cube(
						Aabb3::from_origin_size(Vec3::Y * 0.95, Vec3::splat(1.2), Vec3::new(0.5, 0.0, 0.5)),
						Color3::new(0.5, 0.5, 0.5)
					)
					.with_cube(
						Aabb3::from_origin_size(Vec3::new(-0.5, 0.95 + 0.6, 0.6), Vec3::splat(0.3), Vec3::new(0.5, 0.5, 0.5)),
						Color3::new(0.1, 0.1, 1.0)
					)
					.with_cube(
						Aabb3::from_origin_size(Vec3::new(0.5, 0.95 + 0.6, 0.6), Vec3::splat(0.3), Vec3::new(0.5, 0.5, 0.5)),
						Color3::new(0.1, 0.1, 1.0)
					)
				);

				actor_renderer.push_model_instance(
					gfx,
					Affine3A::from_translation(Vec3::Z * 10.0) * Affine3A::from_rotation_y(PI),
				);
				actor_renderer.push_model_instance(
					gfx,
					Affine3A::from_translation(Vec3::Z * -10.0),
				);

				// Setup UI rendering sub-pass
				let mut ui = ImmRenderer::new();
				ui.brush()
					.transformed_rect_after(
						Aabb2::new(-1.0, 1.0, 2.0, -2.0),
						Aabb2::new(0.0, 0.0, viewport_size.x, viewport_size.y),
					)
					.fill_rect(
						Aabb2::from_origin_size(viewport_size / 2.0, Vec2::splat(20.0), Vec2::splat(0.5)),
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

				// Upload actor data
				actor_renderer.upload(gfx, &mut cb);

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

				// Render actors
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
								load: wgpu::LoadOp::Load,
								store: true,
							}),
							stencil_ops: None,
						}),
					});

					actor_pipeline.bind_pipeline(&mut pass);
					actor_uniforms.write_pass_state(&mut pass);
					actor_renderer.render(&mut pass);
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
				drop(ui);

				// Finish rendering
				gfx.queue.submit([cb.finish()]);
				actor_renderer.reset_and_release();
			}
		}
	})
}
