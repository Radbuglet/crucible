use std::cell::RefCell;

use crucible_common::voxel::{
	container::{VoxelChunkData, VoxelWorldData},
	math::{BlockPos, BlockPosExt, ChunkPos},
};
use geode::prelude::*;
use typed_glam::glam::Mat4;
use winit::event::VirtualKeyCode;

use crate::engine::{
	root::{EngineRootBundle, ViewportBundle},
	services::{scene::SceneUpdateHandler, viewport::ViewportRenderHandler},
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
		voxel_data: RefCell<VoxelWorldData>,
		voxel_mesh: RefCell<VoxelWorldMesh>,
		update_handler: dyn SceneUpdateHandler,
		render_handler: dyn ViewportRenderHandler,
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
		// Create voxel services
		let (voxel_uniforms_guard, voxel_uniforms) = {
			// Get dependencies
			let gfx = engine.gfx(s);
			let mut res_mgr = engine.res_mgr(s).borrow_mut();

			// Create `VoxelUniforms`
			VoxelUniforms::new(s, gfx, &mut res_mgr)
				.box_obj(s)
				.to_guard_ref_pair()
		};

		let voxel_data_guard = VoxelWorldData::default().box_obj_rw(s, main_lock);
		let (voxel_mesh_guard, voxel_mesh) = VoxelWorldMesh::default()
			.box_obj_rw(s, main_lock)
			.to_guard_ref_pair();

		let (local_camera_guard, local_camera) = FreeCamController::default()
			.box_obj_rw(s, main_lock)
			.to_guard_ref_pair();

		// Create event handlers
		let update_handler_guard = Obj::new(s, move |s: Session, _me: Entity, engine: Entity| {
			let engine = EngineRootBundle::unchecked_cast(engine);

			let input_tracker = viewport.input_tracker(s).borrow();
			let gfx = engine.gfx(s);

			// Update chunk meshes
			voxel_mesh.borrow_mut(s).update_chunks(s, gfx, None);

			// Update camera
			local_camera.borrow_mut(s).process(InputActions {
				up: input_tracker.key(VirtualKeyCode::E).state(),
				down: input_tracker.key(VirtualKeyCode::Q).state(),
				left: input_tracker.key(VirtualKeyCode::A).state(),
				right: input_tracker.key(VirtualKeyCode::D).state(),
				fore: input_tracker.key(VirtualKeyCode::W).state(),
				back: input_tracker.key(VirtualKeyCode::S).state(),
			})
		})
		.to_unsized::<dyn SceneUpdateHandler>();

		let render_handler_guard = Obj::new(
			s,
			move |frame: Option<wgpu::SurfaceTexture>,
			      s: Session,
			      _me,
			      viewport: Entity,
			      engine_root: Entity| {
				// Acquire services
				let p_voxel_uniforms = voxel_uniforms.get(s);
				let p_local_camera = local_camera.borrow(s);

				let engine_root = EngineRootBundle::unchecked_cast(engine_root);
				let p_gfx = engine_root.gfx(s);
				let mut p_res_mgr = engine_root.res_mgr(s).borrow_mut();

				let viewport = ViewportBundle::unchecked_cast(viewport);
				let p_viewport_handle = viewport.viewport(s).borrow();
				let mut p_depth_texture = viewport.depth_texture(s).borrow_mut();
				let p_input_tracker = viewport.input_tracker(s).borrow();

				// Acquire frame and create a view to it
				let frame = match frame {
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
						},
					);

					pass.set_pipeline(pipeline.get(s));

					// Render mesh
					voxel_mesh
						.borrow_mut(s)
						.render_chunks(s, p_voxel_uniforms, &mut pass);
				}

				// Present and flush
				p_gfx.queue.submit([cb.finish()]);
				frame.present();
			},
		)
		.to_unsized::<dyn ViewportRenderHandler>();

		// Create main entity
		let scene_guard = Self::spawn(
			s,
			GameSceneBundleCtor {
				voxel_uniforms: voxel_uniforms_guard.into(),
				voxel_data: voxel_data_guard.into(),
				voxel_mesh: voxel_mesh_guard.into(),
				update_handler: update_handler_guard.into(),
				render_handler: render_handler_guard.into(),
				local_camera: local_camera_guard.into(),
			},
		);

		// Create starter chunk
		let (chunk_guard, chunk) = ChunkBundle::new(s, main_lock).to_guard_ref_pair();

		scene_guard
			.weak_copy()
			.voxel_data(s)
			.borrow_mut()
			.add_chunk(
				s,
				ChunkPos::new(0, 0, 0),
				scene_guard.weak_copy().raw(),
				chunk_guard.raw(),
			);

		scene_guard
			.weak_copy()
			.voxel_mesh(s)
			.borrow_mut()
			.flag_chunk(s, main_lock, chunk.raw());

		scene_guard
	}
}

impl ChunkBundle {
	pub fn new(s: Session, main_lock: Lock) -> Owned<Self> {
		{
			let (chunk_data_guard, chunk_data) = VoxelChunkData::default()
				.box_obj_in(s, main_lock)
				.to_guard_ref_pair();

			let p_chunk_data = chunk_data.get(s);

			for pos in BlockPos::iter() {
				if fastrand::bool() {
					p_chunk_data.block_state_of(pos).set_material(1);
				}
			}

			ChunkBundle::spawn(
				s,
				ChunkBundleCtor {
					chunk_data: chunk_data_guard.into(),
				},
			)
		}
	}
}
