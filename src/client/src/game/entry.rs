use std::cell::RefCell;

use crucible_common::voxel::{
	data::{
		BlockLocation, BlockState, ChunkFactoryRequest, RayCast, VoxelChunkData, VoxelWorldData,
	},
	math::{BlockFace, EntityVec, WorldVec, WorldVecExt},
};
use crucible_core::c_enum::CEnum;
use geode::prelude::*;
use typed_glam::glam::Mat4;
use winit::event::{MouseButton, VirtualKeyCode};

use crate::engine::{
	root::{EngineRootBundle, ViewportBundle},
	services::{scene::SceneUpdateEvent, viewport::ViewportRenderEvent},
};

use super::{
	player::camera::{FreeCamController, InputActions},
	voxel::{
		mesh::VoxelWorldMesh,
		pipeline::{VoxelRenderingPipelineDesc, VoxelUniforms},
	},
};

component_bundle! {
	pub struct GameSceneBundle(GameSceneBundleCtor) {
		voxel_uniforms: VoxelUniforms,
		voxel_data: VoxelWorldData,
		voxel_mesh: RefCell<VoxelWorldMesh>,
		entry: RefCell<GameSceneEntry>,
		update_handler: dyn EventHandler<SceneUpdateEvent>,
		render_handler: dyn EventHandlerOnce<ViewportRenderEvent>,
		local_camera: RefCell<FreeCamController>,
	}

	pub struct ChunkBundle(ChunkBundleCtor) {
		chunk_data: VoxelChunkData,
	}
}

impl GameSceneBundle {
	pub fn new(
		s: Session,
		engine: EngineRootBundle,
		viewport: ViewportBundle,
		main_lock: Lock,
	) -> Owned<Self> {
		let scene_guard = Entity::new(s);

		// Create voxel services
		let chunk_factory = Obj::new(s, move |s: Session, _req: ChunkFactoryRequest| {
			let chunk = ChunkBundle::new(s, main_lock);
			EntityWith::cast_owned(chunk.raw())
		})
		.unsize();

		let voxel_data_guard = VoxelWorldData::new(scene_guard.weak_copy(), chunk_factory.into())
			.box_obj_in(s, main_lock);
		let voxel_mesh_guard = VoxelWorldMesh::default().box_obj_rw(s, main_lock);
		let voxel_uniforms_guard = {
			let gfx = engine.gfx(s);
			let mut res_mgr = engine.res_mgr(s).borrow_mut();

			VoxelUniforms::new(s, gfx, &mut res_mgr).box_obj(s)
		};

		// Create event handlers
		let local_camera_guard = FreeCamController::default().box_obj_rw(s, main_lock);

		let (entry_guard, entry) = GameSceneEntry {
			viewport,
			has_control: false,
		}
		.box_obj_rw(s, main_lock)
		.to_guard_ref_pair();

		// Create main entity
		let scene_guard = Self::add_onto_owned(
			s,
			scene_guard,
			GameSceneBundleCtor {
				voxel_uniforms: voxel_uniforms_guard.into(),
				voxel_data: voxel_data_guard.into(),
				voxel_mesh: voxel_mesh_guard.into(),
				entry: entry_guard.into(),
				update_handler: entry.unsize_delegate_borrow_mut().into(),
				render_handler: entry.unsize_delegate_borrow().into(),
				local_camera: local_camera_guard.into(),
			},
		);

		scene_guard
	}
}

impl ChunkBundle {
	pub fn new(s: Session, main_lock: Lock) -> Owned<Self> {
		let (chunk_guard, chunk) = Entity::new(s).to_guard_ref_pair();
		let chunk_data_guard = VoxelChunkData::new(chunk).box_obj_in(s, main_lock);

		ChunkBundle::add_onto_owned(
			s,
			chunk_guard,
			ChunkBundleCtor {
				chunk_data: chunk_data_guard.into(),
			},
		)
	}
}

pub struct GameSceneEntry {
	viewport: ViewportBundle,
	has_control: bool,
}

impl EventHandlerMut<SceneUpdateEvent> for GameSceneEntry {
	fn fire(&mut self, s: Session, me: Entity, event: &mut SceneUpdateEvent) {
		// Ensure that we're still connected to a viewport.
		if !self.viewport.raw().is_alive_now(s) {
			return;
		}

		// Get all dependencies
		let me = GameSceneBundle::cast(me);
		let engine = event.engine;
		let engine = EngineRootBundle::cast(engine);

		let p_lock = engine.main_lock(s).weak_copy();
		let p_input_tracker = self.viewport.input_tracker(s).borrow();
		let p_viewport = self.viewport.viewport(s).borrow();
		let p_gfx = engine.gfx(s);

		let mut p_local_camera = me.local_camera(s).borrow_mut();
		let p_world_data = me.voxel_data(s);
		let mut p_world_mesh = me.voxel_mesh(s).borrow_mut();

		// Handle window focus loss
		if !p_input_tracker.has_focus() {
			self.has_control = false;
		}

		// Ensure that our window is still open
		let p_window = match p_viewport.window() {
			Some(window) => window,
			None => return,
		};

		// Handle interactions
		let esc_pressed = p_input_tracker
			.key(VirtualKeyCode::Escape)
			.recently_pressed();

		let left_pressed = p_input_tracker.button(MouseButton::Left).recently_pressed();

		let right_pressed = p_input_tracker
			.button(MouseButton::Right)
			.recently_pressed();

		if esc_pressed && self.has_control {
			p_window.set_cursor_visible(true);
			let _ = p_window.set_cursor_grab(false);
			self.has_control = false;
		}

		if !self.has_control {
			if left_pressed || right_pressed {
				p_window.set_cursor_visible(false);
				let _ = p_window.set_cursor_grab(true);

				self.has_control = true;
			}
		}

		if self.has_control {
			// Update camera
			p_local_camera.handle_mouse_move(p_input_tracker.mouse_delta());

			p_local_camera.process(InputActions {
				up: p_input_tracker.key(VirtualKeyCode::E).state(),
				down: p_input_tracker.key(VirtualKeyCode::Q).state(),
				left: p_input_tracker.key(VirtualKeyCode::A).state(),
				right: p_input_tracker.key(VirtualKeyCode::D).state(),
				fore: p_input_tracker.key(VirtualKeyCode::W).state(),
				back: p_input_tracker.key(VirtualKeyCode::S).state(),
			});

			// Handle block placement
			if right_pressed {
				let mut ray = RayCast::new_cached(
					&p_world_data,
					EntityVec::new_from(p_local_camera.pos().as_dvec3()),
					EntityVec::new_from(p_local_camera.facing().as_dvec3()),
				);

				for isect in ray.step_for(s, 7.) {
					let mut block_loc = isect.block_loc;

					if block_loc
						.get_block_state(s, p_world_data)
						.unwrap_or_default()
						.material != 0
					{
						let mut target_loc = block_loc.neighbor(s, isect.face.invert());

						target_loc.set_block_state(
							s,
							p_world_data,
							BlockState {
								material: 1,
								..Default::default()
							},
						);

						break;
					}
				}
			}

			// Handle block breaking
			if left_pressed {
				let mut ray = RayCast::new_cached(
					&p_world_data,
					EntityVec::new_from(p_local_camera.pos().as_dvec3()),
					EntityVec::new_from(p_local_camera.facing().as_dvec3()),
				);

				for isect in ray.step_for(s, 100.) {
					let mut block_loc = isect.block_loc;
					let chunk = match block_loc.chunk(s, &p_world_data) {
						Some(chunk) => chunk,
						None => continue,
					};

					if chunk
						.comp(s)
						.get_block_state(block_loc.vec().block())
						.material != 0
					{
						chunk.comp(s).set_block_state(
							s,
							block_loc.vec().block(),
							BlockState::default(),
						);

						break;
					}
				}
			}
		}

		if p_input_tracker.key(VirtualKeyCode::Key1).state() {
			let pos = p_local_camera.pos();
			let pos = WorldVec::new_from(pos.floor().as_ivec3());
			let pos = BlockLocation::new_uncached(pos);

			for x in -3..3 {
				for y in -3..3 {
					for z in -3..3 {
						let mut pos = pos.move_by(s, WorldVec::new(x, y, z));

						let chunk = pos.chunk_or_add(s, &p_world_data);
						chunk.comp(s).set_block_state(
							s,
							pos.vec().block(),
							BlockState {
								material: 1,
								..Default::default()
							},
						);
					}
				}
			}
		}

		// Update chunk meshes
		for chunk in p_world_data.flush_dirty_chunks() {
			p_world_mesh.flag_chunk(s, p_lock, chunk.raw());

			// TODO: Make this more conservative.
			for face in BlockFace::variants() {
				let neighbor = match chunk.comp(s).neighbor(face) {
					Some(neighbor) => neighbor,
					None => continue,
				};

				p_world_mesh.flag_chunk(s, p_lock, neighbor.raw());
			}
		}

		p_world_mesh.update_chunks(s, p_gfx, None);
	}
}

impl EventHandlerOnce<ViewportRenderEvent> for GameSceneEntry {
	fn fire(&self, s: Session, me: Entity, event: ViewportRenderEvent) {
		// Acquire services
		let me = GameSceneBundle::cast(me);

		let p_voxel_uniforms = me.voxel_uniforms(s);
		let p_local_camera = me.local_camera(s).borrow();

		let engine = EngineRootBundle::cast(event.engine);
		let p_gfx = engine.gfx(s);
		let mut p_res_mgr = engine.res_mgr(s).borrow_mut();

		let viewport = ViewportBundle::cast(event.viewport);
		let p_viewport_handle = viewport.viewport(s).borrow();
		let mut p_depth_texture = viewport.depth_texture(s).borrow_mut();
		let p_input_tracker = viewport.input_tracker(s).borrow();

		// Acquire frame and create a view to it
		let frame = match event.frame {
			Some(frame) => frame,
			None => return,
		};
		let frame_view = frame.texture.create_view(&Default::default());

		// Acquire depth texture
		let depth_tex_format = p_depth_texture.format();
		let depth_tex_view = p_depth_texture
			.acquire(p_gfx, &*p_viewport_handle)
			.unwrap()
			.1;

		// Setup projection matrix
		{
			let aspect = p_viewport_handle.surface_aspect().unwrap_or(1.);
			let proj = Mat4::perspective_lh(70f32.to_radians(), aspect, 0.1, 100.);
			let view = p_local_camera.view_matrix();
			// let view = view.inverse();  (already inversed by `look_at_lh`)
			let full = proj * view;

			p_voxel_uniforms.set_camera_matrix(p_gfx, full);
		}

		// Encode main pass
		let mut cb = p_gfx.device.create_command_encoder(&Default::default());
		{
			// Begin pass
			let mut pass = cb.begin_render_pass(&wgpu::RenderPassDescriptor {
				label: None,
				color_attachments: &[Some(wgpu::RenderPassColorAttachment {
					view: &frame_view,
					resolve_target: None,
					ops: wgpu::Operations {
						load: wgpu::LoadOp::Clear(wgpu::Color {
							r: 90. / 255.,
							g: 184. / 255.,
							b: 224. / 255.,
							a: 1.0,
						}),
						store: true,
					},
				})],
				depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
					view: depth_tex_view,
					depth_ops: Some(wgpu::Operations {
						load: wgpu::LoadOp::Clear(f32::INFINITY),
						store: true,
					}),
					stencil_ops: None,
				}),
			});

			// Set appropriate pipeline
			let pipeline = p_res_mgr.load(
				s,
				p_gfx,
				VoxelRenderingPipelineDesc {
					surface_format: p_viewport_handle.format(),
					depth_format: depth_tex_format,
					is_wireframe: p_input_tracker.key(VirtualKeyCode::Space).state(),
					back_face_culling: true,
				},
			);

			pass.set_pipeline(pipeline.resource(s));

			// Render mesh
			me.voxel_mesh(s)
				.borrow_mut()
				.render_chunks(s, p_voxel_uniforms, &mut pass);
		}

		// Present and flush
		p_gfx.queue.submit([cb.finish()]);
		frame.present();
	}
}
