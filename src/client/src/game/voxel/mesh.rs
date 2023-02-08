use std::{
	cell::Ref,
	collections::HashSet,
	time::{Duration, Instant},
};

use crucible_common::{
	game::material::MaterialRegistry,
	voxel::{
		data::VoxelChunkData,
		math::{BlockFace, BlockVec, BlockVecExt, WorldVec, WorldVecExt},
	},
};
use crucible_util::{lang::polyfill::OptionPoly, mem::c_enum::CEnum};
use geode::{storage, Entity};
use wgpu::util::DeviceExt;

use crate::engine::{gfx::atlas::AtlasTexture, io::gfx::GfxContext};

use super::{
	material::BlockDescriptorVisual,
	pipeline::{VoxelUniforms, VoxelVertex},
};

#[derive(Debug, Default)]
pub struct VoxelWorldMesh {
	rendered_chunks: HashSet<Entity>,
	dirty_queue: Vec<Entity>,
}

impl VoxelWorldMesh {
	pub fn flag_chunk(&mut self, chunk: Entity) {
		if let Some(mut chunk_mesh) = chunk.try_get_mut::<VoxelChunkMesh>() {
			if chunk_mesh.still_dirty {
				return;
			}
			chunk_mesh.still_dirty = true;
		}

		self.dirty_queue.push(chunk);
	}

	pub fn update_chunks(
		&mut self,
		gfx: &GfxContext,
		atlas: &AtlasTexture,
		registry: &MaterialRegistry,
		time_limit: Option<Duration>,
	) {
		let started = Instant::now();

		let datas = storage::<VoxelChunkData>();
		let descriptors = storage::<BlockDescriptorVisual>();

		while let Some(chunk) = self.dirty_queue.pop() {
			// Acquire dependencies
			let chunk_data = datas.get(chunk);

			// Mesh chunk
			let mut vertices = Vec::new();

			for center_pos in BlockVec::iter() {
				// Process material
				let material = chunk_data.block_state(center_pos).material;
				if material == 0 {
					continue;
				}

				let material_desc = registry.resolve_slot(material);
				let atlas_tile = descriptors.get(material_desc).atlas_tile;
				let uv_bounds = atlas.decode_uv_bounds(atlas_tile);

				// For every side of a solid block...
				for face in BlockFace::variants() {
					let neighbor_block = center_pos + face.unit();

					// If the neighbor isn't solid...
					let is_solid = if neighbor_block.is_valid() {
						chunk_data.block_state(neighbor_block).material != 0
					} else {
						chunk_data.neighbor(face).p_is_some_and(|neighbor| {
							datas
								.get(neighbor)
								.block_state(neighbor_block.wrap())
								.material != 0
						})
					};

					if is_solid {
						continue;
					}

					// Mesh it!
					let center_pos = WorldVec::compose(chunk_data.pos(), center_pos);
					VoxelVertex::push_quad(
						&mut vertices,
						center_pos.to_glam().as_vec3(),
						face,
						uv_bounds,
					);
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

			chunk.insert(VoxelChunkMesh {
				still_dirty: false,
				vertex_count: vertices.len() as u32,
				buffer,
			});

			self.rendered_chunks.insert(chunk);

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

	#[must_use]
	pub fn prepare_chunk_draw_pass(&self) -> ChunkRenderPass {
		let meshes = storage::<VoxelChunkMesh>();

		ChunkRenderPass {
			meshes: self
				.rendered_chunks
				.iter()
				.map(|&chunk| meshes.get(chunk))
				.collect(),
		}
	}
}

#[derive(Debug)]
pub struct ChunkRenderPass {
	meshes: Vec<Ref<'static, VoxelChunkMesh>>,
}

impl ChunkRenderPass {
	pub fn push<'a>(&'a self, voxel_uniforms: &'a VoxelUniforms, pass: &mut wgpu::RenderPass<'a>) {
		voxel_uniforms.write_pass_state(pass);

		for mesh in &self.meshes {
			let Some(buffer) = &mesh.buffer else {
				continue;
			};

			pass.set_vertex_buffer(0, buffer.slice(..));
			pass.draw(0..mesh.vertex_count, 0..1);
		}
	}
}

#[derive(Debug, Default)]
pub struct VoxelChunkMesh {
	still_dirty: bool,
	vertex_count: u32,
	buffer: Option<wgpu::Buffer>,
}
