use crucible_common::voxel::{
	container::{VoxelChunkData, VoxelWorldData},
	math::{BlockPos, ChunkPos},
};
use geode::prelude::*;

use crate::engine::{gfx::GfxContext, scene::SceneUpdateHandler, viewport::ViewportRenderHandler};

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
		      _viewport,
		      engine_root: Entity| {
			// Acquire services
			let gfx = engine_root.get::<GfxContext>(s);

			// Acquire frame and create a view to it.
			let frame = match frame {
				Some(frame) => frame,
				None => return,
			};

			let frame_view = frame.texture.create_view(&Default::default());

			// Encode main pass
			let mut cb = gfx.device.create_command_encoder(&Default::default());
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
					.render_chunks(s, voxel_pipeline.get(s), &mut pass);
			}

			// Present and flush
			gfx.queue.submit([cb.finish()]);
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
