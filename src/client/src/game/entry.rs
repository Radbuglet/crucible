use crucible_common::voxel::{
	container::{VoxelChunkData, VoxelWorldData},
	math::{BlockPos, ChunkPos},
};
use geode::prelude::*;
use typed_glam::glam::{Mat4, Vec3};

use crate::engine::{
	gfx::GfxContext,
	scene::SceneUpdateHandler,
	viewport::{Viewport, ViewportRenderHandler},
};

use super::voxel::{mesh::VoxelWorldMesh, pipeline::VoxelRenderingPipeline};

pub fn make_game_entry(s: Session, engine_root: Entity, main_lock: Lock) -> Owned<Entity> {
	// Create voxel services
	let voxel_pipeline_guard =
		VoxelRenderingPipeline::new(s, engine_root.get::<GfxContext>(s)).box_obj(s);

	let voxel_pipeline = *voxel_pipeline_guard;

	let voxel_world_data_guard = VoxelWorldData::default().box_obj_rw(s, main_lock);
	let voxel_world_mesh_guard = VoxelWorldMesh::default().box_obj_rw(s, main_lock);
	let voxel_world_mesh = *voxel_world_mesh_guard;

	{
		let chunk_entity_guard = Entity::new(s);
		let chunk_entity = *chunk_entity_guard;

		let chunk_data_guard = VoxelChunkData::default().box_obj_in(s, main_lock);
		let chunk_data = *chunk_data_guard;

		chunk_entity.add(s, chunk_data_guard);

		chunk_data
			.get(s)
			.block_state_of(BlockPos::new(0, 0, 0))
			.set_material(1);

		voxel_world_data_guard.borrow_mut(s).add_chunk(
			s,
			ChunkPos::new(0, 0, 0),
			engine_root,
			chunk_entity_guard,
		);

		voxel_world_mesh
			.borrow_mut(s)
			.flag_chunk(s, main_lock, chunk_entity);
	}

	// Create event handlers
	let update_handler_guard = Obj::new(s, move |s: Session, _me: Entity, engine_root: Entity| {
		let gfx = engine_root.get::<GfxContext>(s);
		voxel_world_mesh.borrow_mut(s).update_chunks(s, &gfx, None);
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
			let p_gfx = engine_root.get::<GfxContext>(s);
			let p_voxel_pipeline = voxel_pipeline.get(s);
			let p_viewport_handle = viewport.borrow::<Viewport>(s);

			// Acquire frame and create a view to it
			let frame = match frame {
				Some(frame) => frame,
				None => return,
			};

			let frame_view = frame.texture.create_view(&Default::default());

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
					depth_stencil_attachment: None,
				});

				voxel_world_mesh
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
	Entity::new_with(
		s,
		(
			update_handler_guard,
			render_handler_guard,
			voxel_pipeline_guard,
			voxel_world_data_guard,
			voxel_world_mesh_guard,
		),
	)
}
