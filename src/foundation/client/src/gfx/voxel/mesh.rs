use std::time::{Duration, Instant};

use bort::{
	saddle::{cx, BortComponents},
	storage, CompRef, Entity,
};
use crevice::std430::AsStd430;
use crucible_foundation_shared::{
	material::{MaterialId, MaterialRegistry},
	math::{AaQuad, BlockFace, BlockVec, BlockVecExt, Sign, Tri, WorldVec, WorldVecExt, QUAD_UVS},
	voxel::{
		data::{self, WorldVoxelData},
		mesh::QuadMeshLayer,
	},
};
use crucible_util::mem::{
	array::map_arr,
	c_enum::{CEnum, CEnumMap},
};
use hashbrown::HashSet;
use typed_glam::glam::{UVec2, Vec3};
use wgpu::util::DeviceExt;

use crate::engine::{gfx::atlas::AtlasTexture, io::gfx::GfxContext};

use super::pipeline::{VoxelUniforms, VoxelVertex};

// === Context === //

cx! {
	pub trait CxMut(BortComponents): data::CxRef;
}

// === Services === //

#[derive(Debug, Default)]
pub struct WorldVoxelMesh {
	rendered_chunks: HashSet<Entity>,
	dirty_queue: Vec<Entity>,
}

impl WorldVoxelMesh {
	pub fn flag_chunk(&mut self, chunk: Entity) {
		if let Some(mut chunk_mesh) = chunk.try_get_mut::<ChunkVoxelMesh>() {
			if chunk_mesh.still_dirty {
				return;
			}
			chunk_mesh.still_dirty = true;
		}

		self.dirty_queue.push(chunk);
	}

	pub fn update_chunks(
		&mut self,
		cx: &impl CxMut,
		world: &WorldVoxelData,
		gfx: &GfxContext,
		atlas: &AtlasTexture,
		registry: &MaterialRegistry,
		time_limit: Option<Duration>,
	) {
		let started = Instant::now();

		let descriptors = storage::<BlockDescriptorVisual>();

		while let Some(chunk) = self.dirty_queue.pop() {
			// Ignore dead chunks
			if !chunk.is_alive() {
				continue;
			}

			// Acquire dependencies
			let chunk_data = world.read_chunk(cx, chunk.obj());

			// Mesh chunk
			let mut vertices = Vec::new();

			for center_pos in BlockVec::iter() {
				// Decode material
				let material = chunk_data.block_or_air(center_pos).material;
				if material == MaterialId::AIR {
					continue;
				}
				let material = registry.find_by_id(material);

				// Determine the center block mesh origin
				// (this is used by all three branches)
				let center_origin = WorldVec::compose(chunk_data.pos(), center_pos)
					.to_glam()
					.as_vec3();

				// Process material
				match &*descriptors.get(material.descriptor) {
					BlockDescriptorVisual::Cubic { textures } => {
						// For every side of a solid block...
						for face in BlockFace::variants() {
							let neighbor_block = center_pos + face.unit();

							// If the neighbor isn't solid...
							let is_solid = 'a: {
								let state = if neighbor_block.is_valid() {
									chunk_data.block_or_air(neighbor_block)
								} else {
									let Some(neighbor) = chunk_data.neighbor(face) else {
										break 'a false;
									};

									world
										.read_chunk(cx, neighbor)
										.block_or_air(neighbor_block.wrap())
								};

								if state.is_air() {
									break 'a false;
								}

								let material = registry.find_by_id(state.material);
								let descriptor = descriptors.get(material.descriptor);

								matches!(&*descriptor, BlockDescriptorVisual::Cubic { .. })
							};

							if is_solid {
								continue;
							}

							// Mesh it!
							{
								// Decode the texture bounds
								let (uv_origin, uv_size) = atlas.decode_uv_bounds(textures[face]);

								// Determine the quad origin
								let center_origin = if face.sign() == Sign::Positive {
									center_origin + face.axis().unit_typed::<Vec3>()
								} else {
									center_origin
								};

								// Construct the quad
								let quad = AaQuad::new_unit(center_origin, face);
								let quad = quad
									.as_quad_ccw()
									.zip(QUAD_UVS.map(|v| uv_origin + v * uv_size));

								let [Tri([a, b, c]), Tri([d, e, f])] = quad.to_tris();
								let quad_vertices = [a, b, c, d, e, f];

								// Write the quad
								let quad_vertices = map_arr(quad_vertices, |(position, uv)| {
									VoxelVertex { position, uv }.as_std430()
								});

								vertices.extend(quad_vertices);
							}
						}
					}
					BlockDescriptorVisual::Mesh { mesh } => {
						// Push the mesh
						for (quad, material) in mesh.iter_cloned() {
							// Translate the quad relative to the block
							let quad = quad.translated(center_origin);

							// Decode the texture bounds
							let (uv_origin, uv_size) = atlas.decode_uv_bounds(material);

							// Give it UVs
							let quad = quad
								.as_quad_ccw()
								.zip(QUAD_UVS.map(|v| uv_origin + v * uv_size));

							// Convert to triangles
							let [Tri([a, b, c]), Tri([d, e, f])] = quad.to_tris();
							let quad_vertices = [a, b, c, d, e, f];

							// Convert to std340
							let quad_vertices = map_arr(quad_vertices, |(position, uv)| {
								VoxelVertex { position, uv }.as_std430()
							});

							// Write to the vertex buffer
							vertices.extend(quad_vertices);
						}
					}
					BlockDescriptorVisual::Custom => todo!(),
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

			chunk.insert(ChunkVoxelMesh {
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
			if time_limit.is_some_and(|time_limit| started.elapsed() > time_limit) {
				break;
			}
		}
	}

	#[must_use]
	pub fn prepare_chunk_draw_pass(&self) -> ChunkRenderPass {
		let meshes = storage::<ChunkVoxelMesh>();

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
	meshes: Vec<CompRef<'static, ChunkVoxelMesh>>,
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
pub struct ChunkVoxelMesh {
	still_dirty: bool,
	vertex_count: u32,
	buffer: Option<wgpu::Buffer>,
}

// === Material descriptors === //

#[derive(Debug)]
pub enum BlockDescriptorVisual {
	Cubic {
		textures: CEnumMap<BlockFace, UVec2>,
	},
	Mesh {
		mesh: QuadMeshLayer<UVec2>,
	},
	Custom,
}

impl BlockDescriptorVisual {
	pub fn cubic_simple(atlas: UVec2) -> Self {
		Self::Cubic {
			textures: CEnumMap::new([atlas; BlockFace::COUNT]),
		}
	}
}
