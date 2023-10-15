use std::time::Duration;

use bort::{alias, cx, scope, BehaviorRegistry, Cx, Entity, EventGroup, OwnedEntity, Scope};
use crucible_foundation_client::{
	engine::{
		assets::AssetManager,
		gfx::{atlas::AtlasTexture, camera::CameraManager, texture::FullScreenTexture},
		io::{gfx::GfxContext, input::InputManager, viewport::Viewport},
	},
	gfx::{
		actor::{
			manager::{MeshManager, MeshRegistry},
			pipeline::{load_opaque_actor_pipeline, ActorRenderingUniforms},
			renderer::{ActorMeshLayer, ActorRenderer},
		},
		skybox::pipeline::{load_skybox_pipeline, SkyboxUniforms},
		ui::brush::ImmRenderer,
		voxel::{
			mesh::WorldVoxelMesh,
			pipeline::{load_opaque_block_pipeline, VoxelUniforms},
		},
	},
};
use crucible_foundation_shared::{
	actor::{collider::ColliderManager, manager::ActorManager, spatial::Spatial},
	humanoid::item::ItemMaterialRegistry,
	math::{Aabb2, Aabb3, BlockFace, ChunkVec, WorldVec, WorldVecExt},
	voxel::{
		data::{Block, BlockMaterialRegistry, BlockVoxelPointer, ChunkVoxelData, WorldVoxelData},
		loader::{LoadedChunk, WorldLoader},
	},
};
use crucible_util::mem::c_enum::CEnum;
use typed_glam::glam::Vec4;
use winit::{
	event::{MouseButton, VirtualKeyCode},
	window::CursorGrabMode,
};

use crate::{
	entry::{SceneInitScope, SceneRenderHandler, SceneUpdateHandler},
	game::content::player::spawn_local_player,
};

use super::behaviors::{
	InitGame, RenderDrawUiBehavior, RenderProvideCameraBehavior, UpdateApplyPhysics,
	UpdateApplySpatialConstraints, UpdateHandleEarlyEvents, UpdateHandleInputs, UpdatePrePhysics,
	UpdatePropagateSpatials, UpdateTickReset,
};

// === Behaviors === //

alias! {
	let asset_mgr: AssetManager;
	let actor_mgr: ActorManager;
	let actor_uniforms: ActorRenderingUniforms;
	let actor_renderer: ActorRenderer;
	let actor_mesh_manager: MeshManager;
	let atlas_texture: AtlasTexture;
	let bhv: BehaviorRegistry;
	let block_registry: BlockMaterialRegistry;
	let camera_mgr: CameraManager;
	let gfx: GfxContext;
	let input_mgr: InputManager;
	let item_registry: ItemMaterialRegistry;
	let mesh_registry: MeshRegistry;
	let skybox_uniforms: SkyboxUniforms;
	let collider_mgr: ColliderManager;
	let state: GameSceneRoot;
	let viewport_data: Viewport;
	let viewport_depth: FullScreenTexture;
	let voxel_uniforms: VoxelUniforms;
	let world_data: WorldVoxelData;
	let world_loader: WorldLoader;
	let world_mesh: WorldVoxelMesh;
}

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
	s: &mut SceneInitScope,
	engine: Entity,
	viewport: Entity,
) -> OwnedEntity {
	// Spawn base entity
	let root = OwnedEntity::new()
		.with_debug_label("game scene root")
		.with(GameSceneRoot { engine, viewport })
		.with(make_scene_update_handler())
		.with(make_scene_render_handler());

	scope! {
		use s, inject { ref bhv = engine }:
		bhv
			.get::<InitGame>()
			.execute(|delegate, scene| delegate(
				bhv,
				s.decl_call(),
				scene,
				engine,
			),
			root.entity(),
		);
	}

	// Setup initial scene
	scope! {
		use s,
			access cx: Cx<&mut LoadedChunk, &mut ChunkVoxelData>,
			inject {
				mut actor_mgr = root,
				ref bhv = engine,
				mut world_loader = root,
				mut world_data = root,
				ref block_registry = root,
				ref mesh_registry = root,
				ref item_registry = root,
			}:

		// Populate initial world data
		world_loader.temp_load_region(
			cx!(cx),
			world_data,
			Aabb3::from_corners_max_excl(
				WorldVec::new(-100, -50, -100).chunk(),
				WorldVec::new(100, 50, 100).chunk() + ChunkVec::ONE,
			),
		);

		let mut pointer = BlockVoxelPointer::new(&world_data, WorldVec::ZERO);
		let proto_mat = block_registry.find_by_name("crucible:proto").unwrap();

		for x in -100..=100 {
			for y in -50..0 {
				for z in -100..=100 {
					pointer.set_pos(Some((cx!(cx), world_data)), WorldVec::new(x, y, z));
					pointer.set_state_or_warn(cx!(cx), world_data, Block::new(proto_mat.id));
				}
			}
		}

		// Create player
		let mut events = EventGroup::new();
		let mut on_inventory_changed = |_, _, _| {};  // (no subscribers have been set up for this event)
		let player = spawn_local_player(
			actor_mgr,
			mesh_registry,
			item_registry,
			&mut events,
			&mut on_inventory_changed,
		);
		actor_mgr.spawn(&mut events, player);
		bhv.get::<UpdateHandleEarlyEvents>()(bhv, s.decl_call(), &mut events, root.entity());
	}

	root
}

// === Handlers === //

fn make_scene_update_handler() -> SceneUpdateHandler {
	SceneUpdateHandler::new(|bhv, s, me, _main_loop| {
		// Pre-fetch all required context
		scope! {
			use s, inject { ref state = me, ref actor_mgr = me }:
			let main_viewport = state.viewport;
			let actor_tag = actor_mgr.tag();
		}

		// Define an event group
		let mut events = EventGroup::new();

		// Reset actor physics
		scope! { use s:
			bhv.get::<UpdateTickReset>()(bhv, s.decl_call(), &mut events, actor_tag);
		}

		// Process inputs
		scope! {
			use s, inject { ref viewport_data = main_viewport, ref input_mgr = main_viewport }:
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
			bhv.get::<UpdateHandleInputs>()(bhv, s.decl_call(), &mut events, me, actor_tag, &input_mgr);
		}

		// Allow actors to influence their own physics states
		bhv.get::<UpdatePrePhysics>()(bhv, s.decl_call(), &mut events, me);

		// Apply actor physical states
		scope! { use s, inject { ref world_data = me, ref block_registry = me }:
			bhv.get::<UpdateApplyPhysics>()(
				bhv,
				s.decl_call(),
				actor_tag,
				world_data,
				block_registry,
			);
		}

		// Update spatials in response to this update
		bhv.get::<UpdateApplySpatialConstraints>()(bhv, s.decl_call(), me);
		bhv.get::<UpdatePropagateSpatials>()(bhv, s.decl_call(), me);
	})
}

fn make_scene_render_handler() -> SceneRenderHandler {
	SceneRenderHandler::new(|bhv, s, me, viewport, frame| {
		scope! { use s, access cx: Cx<&Viewport>, inject { ref state = me, ref actor_mgr = me }:
			let engine = state.engine;
			let main_viewport = state.viewport;
			let actor_tag = actor_mgr.tag();

			if viewport != main_viewport {
				return;
			}

			let Some(viewport_size) = viewport.get_s::<Viewport>(cx!(cx)).curr_surface_size() else { return };
			let viewport_size = viewport_size.as_vec2();
			let Some(aspect) = viewport.get_s::<Viewport>(cx!(cx)).curr_surface_aspect() else { return };
		}

		scope! {
			use s,
				access cx: Cx<&mut ChunkVoxelData>,
				inject {
					ref gfx = engine,
					mut world_data = me,
					mut world_mesh = me,
					ref atlas_texture = me,
					ref block_registry = me,
				}:

			// Consume flagged chunks
			#[clippy::accept_danger(direct_voxel_data_flush, reason = "this is that system!")]
			let dirty_chunks = world_data.flush_dirty(cx!(cx));

			for dirty in dirty_chunks {
				world_mesh.flag_chunk(dirty.entity());

				for neighbor in BlockFace::variants() {
					let Some(neighbor) = world_data
						.read_chunk(cx!(cx), dirty)
						.neighbor(neighbor)
					else {
						continue
					};
					world_mesh.flag_chunk(neighbor.entity());
				}
			}

			// Update the world
			world_mesh.update_chunks(
				cx!(cx),
				world_data,
				gfx,
				atlas_texture,
				block_registry,
				Some(Duration::from_millis(16)),
			);
		}

		scope! {
			use s, access cx: Cx<&mut CameraManager>, inject { mut camera_mgr = me }:

			// Determine the active camera
			camera_mgr.unset();
			bhv.get::<RenderProvideCameraBehavior>()(
				bhv,
				s.decl_call(),
				actor_tag,
				camera_mgr,
			);

			let camera_mgr_snap = camera_mgr.clone();
		}

		scope! {
			use s:
			// Setup UI rendering sub-pass
			let mut ui = ImmRenderer::new();
			let mut brush = ui.brush()
				.transformed_rect_after(
					Aabb2::new(-1.0, 1.0, 2.0, -2.0),
					Aabb2::new(0.0, 0.0, viewport_size.x, viewport_size.y),
				);

			bhv.get::<RenderDrawUiBehavior>()(bhv, s.decl_call(), &mut brush, viewport_size, me);
		}

		scope! {
			use s,
				access cx: Cx<&ActorMeshLayer, &Spatial>,
				inject {
					ref gfx = engine,
					mut actor_renderer = me,
					ref actor_uniforms = me,
					mut asset_mgr = engine,
					ref viewport_data = viewport,
					mut viewport_depth = viewport,
					mut world_mesh = me,
					mut skybox_uniforms = me,
					mut voxel_uniforms = me,
					ref actor_mesh_manager = me,
				}:

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
			actor_mesh_manager.render(cx!(cx), gfx, actor_renderer);
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
	})
}
