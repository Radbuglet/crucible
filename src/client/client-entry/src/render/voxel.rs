use std::{num::NonZeroU64, sync::Arc, time::Duration};

use bevy_autoken::{random_component, Obj, RandomAccess, RandomEntityExt};
use bevy_ecs::{event::EventReader, query::With, system::Query};
use crevice::std430::AsStd430 as _;
use crucible_assets::AssetManager;
use crucible_math::{
    AaQuad, BlockFace, BlockVec, BlockVecExt as _, Sign, Tri, WorldVec, WorldVecExt as _, QUAD_UVS,
};
use crucible_utils::{
    hash::FxHashSet,
    newtypes::{EnumIndex as _, IndexArray},
};
use crucible_world::{
    material::MaterialCache,
    mesh::QuadMeshLayer,
    voxel::{
        BlockMaterial, BlockMaterialCache, BlockMaterialRegistry, ChunkQueue, ChunkVoxelData,
        WorldChunkCreated, WorldPointer, WorldVoxelData,
    },
};
use main_loop::GfxContext;
use typed_glam::glam::{UVec2, Vec3};
use typed_wgpu::BufferBinding;

use super::{
    helpers::{AtlasTexture, BindGroupExt, MultiPass},
    pipelines::voxel::{
        VoxelChunkInstanceBindGroup, VoxelChunkUniformData, VoxelCsmPipeline, VoxelOpaquePipeline,
        VoxelUniforms, VoxelVertex,
    },
};

// === WorldVoxelMesh === //

#[derive(Debug)]
pub struct WorldVoxelMesh {
    material_cache: BlockMaterialCache<MaterialVisualDescriptor>,
    rendered_chunks: FxHashSet<Obj<ChunkVoxelMesh>>,
    dirty_queue: ChunkQueue<Obj<ChunkVoxelMesh>>,
}

random_component!(WorldVoxelMesh);

impl WorldVoxelMesh {
    pub fn new(registry: Obj<BlockMaterialRegistry>) -> Self {
        Self {
            material_cache: MaterialCache::new(registry),
            rendered_chunks: FxHashSet::default(),
            dirty_queue: ChunkQueue::default(),
        }
    }

    pub fn update(&mut self, gfx: &GfxContext, atlas: &AtlasTexture, time_limit: Option<Duration>) {
        if !self.dirty_queue.is_empty() {
            tracing::info!("Dirty chunk count: {}", self.dirty_queue.len());
        }

        cbit::cbit!(for mut chunk in self.dirty_queue.process(time_limit) {
            // Ensure that the chunk is still alive
            if !chunk.is_alive() {
                continue;
            }

            // Ensure that the chunk is only re-rendered once
            if !chunk.dirty {
                continue;
            }

            chunk.dirty = false;

            let data = &*chunk.data();

            let mut vertices = Vec::new();

            for center_pos in BlockVec::iter() {
                // Decode material
                let material = data.block_or_air(center_pos).material;
                if material == BlockMaterial::AIR {
                    continue;
                }
                let material = self.material_cache.get(material).unwrap();

                // Determine the center block mesh origin
                // (this is used by all three branches)
                let center_origin = WorldVec::compose(data.pos(), center_pos)
                    .to_glam()
                    .as_vec3();

                // Process material
                match &*material {
                    MaterialVisualDescriptor::Cubic { textures } => {
                        // For every side of a solid block...
                        for face in BlockFace::variants() {
                            let neighbor_block = center_pos + face.unit();

                            // If the neighbor isn't solid...
                            let is_solid = 'a: {
                                let state = if neighbor_block.is_valid() {
                                    data.block_or_air(neighbor_block)
                                } else {
                                    let Some(neighbor) = data.neighbor(face) else {
                                        break 'a false;
                                    };

                                    neighbor.block_or_air(neighbor_block.wrap())
                                };

                                if state.is_air() {
                                    break 'a false;
                                }

                                let material = self.material_cache.get(state.material).unwrap();

                                matches!(&*material, MaterialVisualDescriptor::Cubic { .. })
                            };

                            if is_solid {
                                continue;
                            }

                            // Mesh it!
                            {
                                // Decode the texture bounds
                                let (uv_origin, uv_size) =
                                    atlas.decode_uv_percent_bounds(textures[face]);

                                // Determine the quad origin
                                let center_origin = if face.sign() == Sign::Positive {
                                    center_origin + face.axis().unit_f()
                                } else {
                                    center_origin
                                };

                                // Construct the quad
                                let quad = AaQuad::new_unit(center_origin, face);
                                let quad = quad
                                    .as_quad_ccw_whmask()
                                    // Determine UV
                                    .zip(QUAD_UVS.map(|v| uv_origin + v * uv_size))
                                    // Determine occlusion
                                    .map(|((pos, whmask), uv)| {
                                        let mut is_occluded = false;
                                        let (h_rel, v_rel) = face.axis().ortho_hv();
                                        let h_rel = h_rel.unit_typed::<WorldVec>()
                                            * if whmask.x { 1 } else { -1 };
                                        let v_rel = v_rel.unit_typed::<WorldVec>()
                                            * if whmask.y { 1 } else { -1 };

                                        let occlude_origin =
                                            WorldVec::compose(data.pos(), center_pos) + face.unit();

                                        for (h_mul, v_mul) in [(1, 0), (0, 1), (1, 1)] {
                                            let rel = h_rel * h_mul + v_rel * v_mul;

                                            is_occluded |= WorldPointer::new(occlude_origin + rel)
                                                .state_or_air(data.world())
                                                .is_not_air();
                                        }

                                        (pos, uv, if is_occluded { 0.8 } else { 1. })
                                    });

                                let [Tri([a, b, c]), Tri([d, e, f])] = quad.to_tris();
                                let quad_vertices = [a, b, c, d, e, f];

                                // Write the quad
                                let quad_vertices = quad_vertices.map(|(position, uv, light)| {
                                    VoxelVertex {
                                        position,
                                        uv,
                                        light,
                                        normal: face.unit_typed(),
                                    }
                                    .as_std430()
                                });

                                vertices.extend(quad_vertices);
                            }
                        }
                    }
                    MaterialVisualDescriptor::Mesh { mesh } => {
                        // Push the mesh
                        for (quad, material) in mesh.iter_cloned() {
                            let normal = quad.face.unit_typed();

                            // Translate the quad relative to the block
                            let quad = quad.translated(center_origin);

                            // Decode the texture bounds
                            let (uv_origin, uv_size) = atlas.decode_uv_percent_bounds(material);

                            // Give it UVs
                            let quad = quad
                                .as_quad_ccw()
                                .zip(QUAD_UVS.map(|v| uv_origin + v * uv_size));

                            // Convert to triangles
                            let [Tri([a, b, c]), Tri([d, e, f])] = quad.to_tris();
                            let quad_vertices = [a, b, c, d, e, f];

                            // Convert to std340
                            let quad_vertices = quad_vertices.map(|(position, uv)| {
                                VoxelVertex {
                                    position,
                                    uv,
                                    light: 1.,
                                    normal,
                                }
                                .as_std430()
                            });

                            // Write to the vertex buffer
                            vertices.extend(quad_vertices);
                        }
                    }
                }
            }

            // Replace the chunk mesh
            let buffer = if !vertices.is_empty() {
                Some(Arc::new(typed_wgpu::Buffer::create_init(
                    &gfx.device,
                    &typed_wgpu::BufferInitDescriptor {
                        label: Some(format!("chunk mesh {:?}", data.pos()).as_str()),
                        usage: wgpu::BufferUsages::VERTEX,
                        contents: &vertices,
                    },
                )))
            } else {
                None
            };

            chunk.buffer = buffer;
            chunk.vertex_count = vertices.len() as u32;

            self.rendered_chunks.insert(chunk);

            // Log some debug info
            tracing::info!(
                "Meshed {} {} for chunk {:?}",
                vertices.len(),
                if vertices.len() == 1 {
                    "vertex"
                } else {
                    "vertices"
                },
                chunk,
            );
        });
    }

    pub fn prepare_pass(&mut self) -> ChunkRenderPass {
        let mut meshes = Vec::new();
        self.rendered_chunks.retain(|chunk| {
            if !chunk.is_alive() {
                return false;
            }

            if let Some(mesh) = &chunk.buffer {
                meshes.push((mesh.clone(), chunk.vertex_count));
            }

            true
        });

        ChunkRenderPass { meshes }
    }
}

#[derive(Debug)]
pub struct ChunkRenderPass {
    meshes: Vec<(Arc<typed_wgpu::Buffer<VoxelVertex>>, u32)>,
}

impl ChunkRenderPass {
    pub fn render_csm<'a>(
        &'a self,
        pipeline: &'a VoxelCsmPipeline,
        uniforms: &'a VoxelUniforms,
        pass: &mut wgpu::RenderPass<'a>,
    ) {
        pipeline.bind_pipeline(pass);
        pipeline.bind_group(pass, uniforms.common_bind_group(), &[]);

        for (mesh, vertex_count) in &self.meshes {
            pipeline.bind_vertex_buffer(pass, mesh.slice(..));
            pass.draw(0..*vertex_count, 0..1);
        }
    }

    pub fn render_opaque<'p>(
        &'p self,
        assets: &AssetManager,
        gfx: &GfxContext,
        pipeline: &'p VoxelOpaquePipeline,
        uniforms: &'p VoxelUniforms,
        pass: &mut MultiPass<'_, 'p>,
    ) {
        let dyn_bind_group = pass.alloc(|buffer| {
            VoxelChunkInstanceBindGroup {
                buffer: BufferBinding::wrap(wgpu::BufferBinding {
                    buffer,
                    offset: 0,
                    size: Some(NonZeroU64::new(256).unwrap()),
                }),
            }
            .load_instance(assets, gfx, ())
        });

        pass.draw(|pass| {
            pipeline.bind_pipeline(pass);
            pipeline.bind_group(pass, uniforms.common_bind_group(), &[]);
            pipeline.bind_group(pass, uniforms.opaque_bind_group(), &[]);
        });

        for (mesh, vertex_count) in &self.meshes {
            let offset = pass
                .write_typed(gfx, || {
                    VoxelChunkUniformData { offset: Vec3::ZERO }.as_std430()
                })
                .as_offset();

            pass.draw(|pass| {
                pipeline.bind_group(pass, dyn_bind_group.unwrap(), &(offset,));
                pipeline.bind_vertex_buffer(pass, mesh.slice(..));
                pass.draw(0..*vertex_count, 0..1);
            });
        }
    }
}

// === ChunkVoxelMesh === //

#[derive(Debug, Default)]
pub struct ChunkVoxelMesh {
    dirty: bool,
    vertex_count: u32,
    buffer: Option<Arc<typed_wgpu::Buffer<VoxelVertex>>>,
}

random_component!(ChunkVoxelMesh);

impl ChunkVoxelMesh {
    pub fn world(self: Obj<Self>) -> Obj<WorldVoxelMesh> {
        self.data().world().obj::<WorldVoxelMesh>()
    }

    pub fn data(self: Obj<Self>) -> Obj<ChunkVoxelData> {
        self.obj::<ChunkVoxelData>()
    }

    pub fn mark_dirty(mut self: Obj<Self>) {
        if self.dirty {
            return;
        }

        self.dirty = true;
        self.world().dirty_queue.push(self);
    }
}

// === Material Descriptors === //

#[derive(Debug)]
pub enum MaterialVisualDescriptor {
    Cubic {
        textures: IndexArray<BlockFace, UVec2>,
    },
    Mesh {
        mesh: QuadMeshLayer<UVec2>,
    },
}

random_component!(MaterialVisualDescriptor);

impl MaterialVisualDescriptor {
    pub fn cubic_simple(atlas: UVec2) -> Self {
        Self::Cubic {
            textures: IndexArray::new([atlas; BlockFace::COUNT]),
        }
    }
}

// === Systems === //

pub fn sys_attach_mesh_to_visual_chunks(
    mut rand: RandomAccess<(
        &WorldVoxelData,
        &WorldVoxelMesh,
        &ChunkVoxelData,
        &mut ChunkVoxelMesh,
    )>,
    mut query: EventReader<WorldChunkCreated>,
) {
    rand.provide(|| {
        for event in query.read() {
            if !event.world.entity().has::<WorldVoxelMesh>() {
                continue;
            }

            event.chunk.entity().insert(ChunkVoxelMesh::default());
        }
    });
}

pub fn sys_queue_dirty_chunks_for_render(
    mut rand: RandomAccess<(
        &ChunkVoxelData,
        &mut ChunkVoxelMesh,
        &mut WorldVoxelMesh,
        &WorldVoxelData,
    )>,
    mut query: Query<&Obj<WorldVoxelData>, With<Obj<WorldVoxelMesh>>>,
) {
    rand.provide(|| {
        for &world in query.iter_mut() {
            for dirty in world.iter_dirty() {
                for face in dirty.dirty_neighbor_mask().iter_ones() {
                    let Some(neighbor) = dirty.neighbor(face) else {
                        continue;
                    };

                    neighbor.obj::<ChunkVoxelMesh>().mark_dirty();
                }

                for rel in dirty.dirty_corner_iter() {
                    let Some(neighbor) = world.get(dirty.pos() + rel) else {
                        continue;
                    };

                    neighbor.obj::<ChunkVoxelMesh>().mark_dirty();
                }

                dirty.obj::<ChunkVoxelMesh>().mark_dirty();
            }
        }
    });
}
