use std::{
	cell::Cell,
	collections::HashSet,
	time::{Duration, Instant},
};

use crucible_common::voxel::{
	data::VoxelChunkData,
	math::{BlockFace, BlockPos, BlockPosExt, WorldPos, WorldPosExt},
};
use crucible_core::c_enum::ExposesVariants;
use geode::{
	entity::key::{dyn_key, TypedKey},
	prelude::*,
};
use wgpu::util::DeviceExt;

use crate::engine::services::gfx::GfxContext;

use super::pipeline::{VoxelUniforms, VoxelVertex};

pub struct VoxelWorldMesh {
	chunks: HashSet<Entity>,
	dirty_queue: Vec<Entity>,
	mesh_meta_key: TypedKey<VoxelChunkMesh>,
}

impl Default for VoxelWorldMesh {
	fn default() -> Self {
		Self {
			chunks: Default::default(),
			dirty_queue: Default::default(),
			mesh_meta_key: dyn_key(),
		}
	}
}

impl VoxelWorldMesh {
	pub fn flag_chunk(&mut self, s: Session, meshing_lock: Lock, chunk: Entity) {
		if let Ok(chunk_mesh) = chunk.fallible_get_in(s, self.mesh_meta_key).ok_or_missing() {
			if !chunk_mesh.still_dirty.get() {
				chunk_mesh.still_dirty.set(true);
				self.dirty_queue.push(chunk);
			}
		} else {
			let chunk_mesh = VoxelChunkMesh {
				still_dirty: Cell::new(true),
				vertex_count: Cell::new(0),
				buffer: Cell::new(None),
			}
			.box_obj_in(s, meshing_lock);

			chunk.add(s, ExposeUsing(chunk_mesh, self.mesh_meta_key));
			self.dirty_queue.push(chunk);
		}
	}

	pub fn update_chunks(&mut self, s: Session, gfx: &GfxContext, time_limit: Option<Duration>) {
		let started = Instant::now();

		for dirty in self.dirty_queue.drain(..) {
			// Acquire instances
			let chunk_data = dirty.get::<VoxelChunkData>(s);
			let chunk_mesh = dirty.get_in::<VoxelChunkMesh>(s, self.mesh_meta_key);

			// Generate mesh
			let vertices = {
				let mut vertices = Vec::new();
				for center_pos in BlockPos::iter() {
					// Don't mesh air blocks
					if chunk_data.get_block(center_pos).material == 0 {
						continue;
					}

					// For every side of a solid block...
					for face in BlockFace::variants() {
						let neighbor_block = center_pos + face.unit();

						// If the neighbor isn't solid...
						let is_solid = if neighbor_block.is_valid() {
							chunk_data.get_block(neighbor_block).material != 0
						} else {
							chunk_data.neighbor(face).map_or(false, |neighbor_entity| {
								let neighbor_chunk = neighbor_entity.get::<VoxelChunkData>(s);

								neighbor_chunk.get_block(center_pos.wrap()).material != 0
							})
						};

						if is_solid {
							continue;
						}

						// Mesh it!
						let center_pos = WorldPos::compose(chunk_data.pos(), center_pos);
						VoxelVertex::push_quad(
							&mut vertices,
							center_pos.into_raw().as_vec3(),
							face,
						);
					}
				}
				vertices
			};

			// Unflag chunk and register it
			chunk_mesh.still_dirty.set(false);
			chunk_mesh.vertex_count.set(vertices.len() as u32);
			self.chunks.insert(dirty);

			// Replace buffer
			chunk_mesh.buffer.replace(Some(
				gfx.device
					.create_buffer_init(&wgpu::util::BufferInitDescriptor {
						label: None,
						contents: bytemuck::cast_slice(vertices.as_slice()),
						usage: wgpu::BufferUsages::VERTEX,
					})
					.box_obj(s),
			));

			// Log some debug info
			log::info!(
				"Meshed {} {} for chunk {:?}",
				vertices.len(),
				if vertices.len() == 1 {
					"vertex"
				} else {
					"vertices"
				},
				dirty,
			);

			// Check if we've elapsed our time limit. Do this at the end of the loop to ensure that
			// at least one chunk has the opportunity to be meshed.
			if time_limit.map_or(false, |limit| started.elapsed() > limit) {
				break;
			}
		}
	}

	pub fn render_chunks<'a>(
		&mut self,
		s: Session<'a>,
		voxel_uniforms: &'a VoxelUniforms,
		pass: &mut wgpu::RenderPass<'a>,
	) {
		voxel_uniforms.set_pass_state(pass);

		for chunk in self.chunks.iter() {
			let mesh = chunk.get_in::<VoxelChunkMesh>(s, self.mesh_meta_key);

			// Ignore empty chunks.
			if mesh.vertex_count.get() == 0 {
				continue;
			}

			// Otherwise, acquire the buffer and render.
			let buffer = mesh
				.buffer
				.get_inner()
				.expect("unmeshed chunk should not be in the chunk set")
				.get(s);

			pass.set_vertex_buffer(0, buffer.slice(..));
			pass.draw(0..mesh.vertex_count.get(), 0..1);
		}
	}
}

struct VoxelChunkMesh {
	still_dirty: Cell<bool>,
	vertex_count: Cell<u32>,
	buffer: Cell<Option<Owned<Obj<wgpu::Buffer>>>>,
}
