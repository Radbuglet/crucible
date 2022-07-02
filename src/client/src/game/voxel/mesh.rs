use std::{cell::Cell, collections::HashSet};

use crucible_common::voxel::container::VoxelWorldData;
use geode::{
	entity::key::{dyn_key, TypedKey},
	prelude::*,
};

use super::pipeline::VoxelRenderingPipeline;

pub struct VoxelMeshRenderer {
	chunks: HashSet<Entity>,
	dirty_queue: Vec<Entity>,
	mesh_meta_key: TypedKey<ChunkMesh>,
}

impl Default for VoxelMeshRenderer {
	fn default() -> Self {
		Self {
			chunks: Default::default(),
			dirty_queue: Default::default(),
			mesh_meta_key: dyn_key(),
		}
	}
}

impl VoxelMeshRenderer {
	pub fn flag_chunk(&mut self, s: Session, chunk: Entity) {}

	pub fn update_chunks(&mut self, s: Session, data: &VoxelWorldData) {}

	pub fn render_chunks<'a>(
		&mut self,
		s: Session<'a>,
		assets: &'a VoxelRenderingPipeline,
		pass: &mut wgpu::RenderPass<'a>,
	) {
		pass.set_pipeline(&assets.opaque_block_pipeline);

		for chunk in self.chunks.iter().copied() {
			let mesh = chunk.get_in(s, self.mesh_meta_key);
			let buffer = mesh.buffer.get().get(s);

			pass.set_vertex_buffer(0, buffer.slice(..));
			pass.draw(0..6, 0..mesh.face_count.get());
		}
	}
}

struct ChunkMesh {
	still_dirty: Cell<bool>,
	face_count: Cell<u32>,
	buffer: Cell<Obj<wgpu::Buffer>>,
}
