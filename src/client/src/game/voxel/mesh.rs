use std::time::{Duration, Instant};

use crevice::std430::AsStd430;
use crucible_common::{
	game::material::MaterialRegistry,
	voxel::{
		data::VoxelChunkData,
		math::{BlockFace, BlockVec, BlockVecExt, Sign, WorldVec, WorldVecExt},
	},
};
use crucible_util::{
	lang::polyfill::OptionPoly,
	mem::{
		array::{map_arr, zip_arr},
		c_enum::CEnum,
	},
};
use geode::{storage, CompRef, Entity};
use hashbrown::HashSet;
use typed_glam::glam::{Vec2, Vec3};
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
					push_quad(
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
	meshes: Vec<CompRef<VoxelChunkMesh>>,
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

fn push_quad(
	target: &mut Vec<<VoxelVertex as AsStd430>::Output>,
	origin: Vec3,
	face: BlockFace,
	(uv_origin, uv_size): (Vec2, Vec2),
) {
	// Determine the quad origin
	let origin = if face.sign() == Sign::Positive {
		origin + face.axis().unit_typed::<Vec3>()
	} else {
		origin
	};

	// Construct the quad
	let quad = geom::aabb_quad(origin, face);
	let quad = zip_arr(quad, map_arr(geom::QUAD_UVS, |v| uv_origin + v * uv_size));
	let [[a, b, c], [d, e, f]] = geom::quad_to_tris(quad);
	let vertices = [a, b, c, d, e, f]; // TODO: use `.flatten` once stabilized

	// Write the quad
	let vertices = map_arr(vertices, |(position, uv)| {
		VoxelVertex { position, uv }.as_std430()
	});

	target.extend(vertices);
}

pub mod geom {
	use crucible_common::voxel::math::{Axis3, BlockFace, Sign};
	use typed_glam::{glam::Vec2, traits::NumericVector3};

	// Quads, from a front-view, are laid out as follows:
	//
	//      d---c
	//      |   |
	//      a---b
	//
	// Textures, meanwhile, are laid out as follows:
	//
	// (0,0)     (1,0)
	//      *---*
	//      |   |
	//      *---*
	// (0,1)     (1,1)
	//
	// Hence:
	pub const QUAD_UVS: [Vec2; 4] = [
		Vec2::new(0.0, 1.0),
		Vec2::new(1.0, 1.0),
		Vec2::new(1.0, 0.0),
		Vec2::new(0.0, 0.0),
	];

	pub fn quad_to_tris<V: Copy>([a, b, c, d]: [V; 4]) -> [[V; 3]; 2] {
		// If we have a quad like this facing us:
		//
		// d---c
		// |   |
		// a---b
		//
		// We can split it up into triangles preserving the winding order like so:
		//
		//       3
		//      c
		//     /|
		//    / |
		//   /  |
		//  a---b
		// 1     2
		//
		// ...and:
		//
		// 3     2
		//  d---c
		//  |  /
		//  | /
		//  |/
		//  a
		// 1
		[[a, b, c], [a, c, d]]
	}

	pub fn flip_quad_winding<V>([a, b, c, d]: [V; 4]) -> [V; 4] {
		// If we have a quad like this facing us:
		//
		// d---c
		// |   |
		// a---b
		//
		// From the other side, it looks like this:
		//
		// c---d
		// |   |
		// b---a
		//
		// If we preserve quad UV rules, we get the new ordering:
		[b, a, d, c]
	}

	pub fn aabb_quad<V: NumericVector3>(origin: V, facing: BlockFace) -> [V; 4] {
		// Our coordinate system is y-up right-handed and looks like this:
		//     +y
		//      |
		// +x---|
		//     /
		//   +z
		//
		let (axis, sign) = facing.decompose();

		// Build the quad with a winding order assumed to be for a negative facing quad.
		let quad = match axis {
			Axis3::X => {
				// A quad facing the negative x direction looks like this:
				//
				//       c +y
				//      /|
				//     / |
				//    d  |
				//    |  b 0
				//    | /
				//    |/
				//    a +z
				//
				[origin + V::Z, origin, origin + V::Y, origin + V::Y + V::Z]
			}
			Axis3::Y => {
				// A quad facing the negative y direction looks like this:
				//
				//  +x        0
				//    d------a
				//   /      /
				//  /      /
				// c------b
				//         +z
				//
				[origin, origin + V::Z, origin + V::X + V::Z, origin + V::X]
			}
			Axis3::Z => {
				// A quad facing the negative z direction looks like this:
				//
				//              +y
				//      c------d
				//      |      |
				//      |      |
				//      b------a
				//    +x        0
				//
				[origin, origin + V::X, origin + V::X + V::Y, origin + V::Y]
			}
		};

		// Flip the winding order if the quad is actually facing the positive direction:
		// FIXME: Why is this the other way around?!
		if sign == Sign::Positive {
			quad
		} else {
			flip_quad_winding(quad)
		}
	}
}
