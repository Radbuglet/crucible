use std::{
	collections::HashSet,
	time::{Duration, Instant},
};

use crucible_common::voxel::{
	data::VoxelChunkData,
	math::{BlockFace, BlockVec, BlockVecExt, WorldVec, WorldVecExt},
};
use crucible_core::{lang::polyfill::OptionPoly, mem::c_enum::CEnum};
use geode::{Dependent, Entity, Storage};
use wgpu::util::DeviceExt;

use crate::engine::io::gfx::GfxContext;

use super::pipeline::{VoxelUniforms, VoxelVertex};

#[derive(Debug, Default)]
pub struct VoxelWorldMesh {
	rendered_chunks: HashSet<Dependent<Entity>>,
	dirty_queue: Vec<Dependent<Entity>>,
}

impl VoxelWorldMesh {
	pub fn flag_chunk(&mut self, (meshes,): (&mut Storage<VoxelChunkMesh>,), chunk: Entity) {
		if let Some(chunk_mesh) = meshes.get_mut(chunk) {
			if chunk_mesh.still_dirty {
				return;
			}
			chunk_mesh.still_dirty = true;
		}

		self.dirty_queue.push(Dependent::new(chunk));
	}

	pub fn update_chunks(
		&mut self,
		(gfx, datas, meshes): (
			&GfxContext,
			&Storage<VoxelChunkData>,
			&mut Storage<VoxelChunkMesh>,
		),
		time_limit: Option<Duration>,
	) {
		let started = Instant::now();

		while let Some(chunk_lt) = self.dirty_queue.pop() {
			// Acquire dependencies
			let chunk = chunk_lt.get();
			let chunk_data = &datas[chunk];

			// Mesh chunk
			let mut vertices = Vec::new();

			for center_pos in BlockVec::iter() {
				// Don't mesh air blocks
				if chunk_data.block_state(center_pos).material == 0 {
					continue;
				}

				// For every side of a solid block...
				for face in BlockFace::variants() {
					let neighbor_block = center_pos + face.unit();

					// If the neighbor isn't solid...
					let is_solid = if neighbor_block.is_valid() {
						chunk_data.block_state(neighbor_block).material != 0
					} else {
						chunk_data.neighbor(face).p_is_some_and(|neighbor| {
							datas[neighbor].block_state(neighbor_block.wrap()).material != 0
						})
					};

					if is_solid {
						continue;
					}

					// Mesh it!
					let center_pos = WorldVec::compose(chunk_data.pos(), center_pos);
					VoxelVertex::push_quad(&mut vertices, center_pos.to_glam().as_vec3(), face);
				}
			}

			// Replace the chunk mesh
			let buffer = if !vertices.is_empty() {
				Some(
					gfx.device
						.create_buffer_init(&wgpu::util::BufferInitDescriptor {
							label: Some(format!("chunk mesh {:?}", chunk_data.pos()).as_str()),
							usage: wgpu::BufferUsages::VERTEX,
							contents: bytemuck::cast_slice(&vertices),
						}),
				)
			} else {
				None
			};

			meshes.insert(
				chunk,
				VoxelChunkMesh {
					still_dirty: false,
					vertex_count: vertices.len() as u32,
					buffer,
				},
			);

			self.rendered_chunks.insert(chunk_lt);

			// Log some debug info
			log::info!(
				"Meshed {} {} for chunk {:?}",
				vertices.len(),
				if vertices.len() == 1 {
					"vertex"
				} else {
					"vertices"
				},
				chunk,
			);

			// Ensure that we haven't gone over our time limit.
			if time_limit.p_is_some_and(|time_limit| started.elapsed() > time_limit) {
				break;
			}
		}
	}

	pub fn render_chunks<'a>(
		&mut self,
		(meshes, voxel_uniforms): (&'a Storage<VoxelChunkMesh>, &'a VoxelUniforms),
		pass: &mut wgpu::RenderPass<'a>,
	) {
		voxel_uniforms.write_pass_state(pass);

		for chunk in &self.rendered_chunks {
			let chunk = chunk.get();
			let chunk_mesh = &meshes[chunk];

			let Some(buffer) = &chunk_mesh.buffer else {
				continue;
			};

			pass.set_vertex_buffer(0, buffer.slice(..));
			pass.draw(0..chunk_mesh.vertex_count, 0..1);
		}
	}
}

#[derive(Debug, Default)]
pub struct VoxelChunkMesh {
	still_dirty: bool,
	vertex_count: u32,
	buffer: Option<wgpu::Buffer>,
}
