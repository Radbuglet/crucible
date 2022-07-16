use std::cell::RefCell;

use crucible_common::voxel::{
	container::{VoxelChunkData, VoxelWorldData},
	math::{BlockPos, ChunkPos},
};
use geode::prelude::*;
use typed_glam::glam::{Mat4, Vec3};

use crate::engine::{
	root::{EngineRootBundle, ViewportBundle},
	services::{gfx::GfxContext, scene::SceneUpdateHandler, viewport::ViewportRenderHandler},
};

use super::voxel::{mesh::VoxelWorldMesh, pipeline::VoxelRenderingPipeline};

component_bundle! {
	pub struct GameSceneBundle(GameSceneBundleCtor) {
		voxel_pipeline: VoxelRenderingPipeline,
		voxel_data: RefCell<VoxelWorldData>,
		voxel_mesh: RefCell<VoxelWorldMesh>,
		update_handler: dyn SceneUpdateHandler,
		render_handler: dyn ViewportRenderHandler,
	}

	pub struct ChunkBundle(ChunkBundleCtor) {
		chunk_data: VoxelChunkData,
	}
}

impl GameSceneBundle {
	pub fn new(s: Session, engine_root: Entity, main_lock: Lock) -> Owned<Self> {
		// Create voxel services
		let (voxel_pipeline_guard, voxel_pipeline) =
			VoxelRenderingPipeline::new(s, engine_root.get::<GfxContext>(s))
				.box_obj(s)
				.to_guard_ref_pair();

		let voxel_data_guard = VoxelWorldData::default().box_obj_rw(s, main_lock);
		let (voxel_mesh_guard, voxel_mesh) = VoxelWorldMesh::default()
			.box_obj_rw(s, main_lock)
			.to_guard_ref_pair();

		// Create event handlers
		let update_handler_guard =
			Obj::new(s, move |s: Session, _me: Entity, engine_root: Entity| {
				let gfx = engine_root.get::<GfxContext>(s);
				voxel_mesh.borrow_mut(s).update_chunks(s, &gfx, None);
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
				let viewport = ViewportBundle::unchecked_cast(viewport);
				let engine_root = EngineRootBundle::unchecked_cast(engine_root);

				let p_gfx = engine_root.gfx(s);
				let p_voxel_pipeline = voxel_pipeline.get(s);
				let p_viewport_handle = viewport.viewport(s).borrow();
				let mut p_depth_texture = viewport.depth_texture(s).borrow_mut();

				// Acquire frame and create a view to it
				let frame = match frame {
					Some(frame) => frame,
					None => return,
				};

				let frame_view = frame.texture.create_view(&Default::default());

				let (_, depth_tex_view) =
					p_depth_texture.acquire(p_gfx, &*p_viewport_handle).unwrap();

				// Setup projection matrix
				{
					let aspect = p_viewport_handle.surface_aspect().unwrap_or(1.);
					let proj = Mat4::perspective_lh(70f32.to_radians(), aspect, 0.1, 100.);
					let view = Mat4::look_at_lh(Vec3::new(-10., -10., -10.), Vec3::ZERO, Vec3::Y);
					// let view = view.inverse();
					let full = proj * view;

					p_voxel_pipeline.set_camera_matrix(p_gfx, full);
				}

				// Encode main pass
				let mut cb = p_gfx.device.create_command_encoder(&Default::default());
				{
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
								load: wgpu::LoadOp::Clear(0.),
								store: true,
							}),
							stencil_ops: None,
						}),
					});

					voxel_mesh
						.borrow_mut(s)
						.render_chunks(s, p_voxel_pipeline, &mut pass);
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
				voxel_pipeline: voxel_pipeline_guard.into(),
				voxel_data: voxel_data_guard.into(),
				voxel_mesh: voxel_mesh_guard.into(),
				update_handler: update_handler_guard.into(),
				render_handler: render_handler_guard.into(),
			},
		);

		// Create starter chunk
		let (chunk_guard, chunk) = ChunkBundle::new(s, main_lock).to_guard_ref_pair();

		scene_guard.voxel_data(s).borrow_mut().add_chunk(
			s,
			ChunkPos::new(0, 0, 0),
			scene_guard.weak_copy().raw(),
			chunk_guard.raw(),
		);

		scene_guard
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

			chunk_data
				.get(s)
				.block_state_of(BlockPos::new(0, 0, 0))
				.set_material(1);

			ChunkBundle::spawn(
				s,
				ChunkBundleCtor {
					chunk_data: chunk_data_guard.into(),
				},
			)
		}
	}
}
