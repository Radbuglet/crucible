use bort::{Entity, OwnedEntity};
use crucible_common::{
	game::material::{MaterialDescriptorBase, MaterialRegistry},
	voxel::{
		data::{VoxelChunkData, VoxelWorldData},
		math::{BlockFace, ChunkVec},
	},
};
use crucible_util::{debug::error::ResultExt, mem::c_enum::CEnum};
use image::Rgba32FImage;
use typed_glam::glam::{UVec2, Vec3};

use crate::engine::{
	assets::AssetManager,
	gfx::{
		atlas::{AtlasTexture, AtlasTextureGfx},
		texture::FullScreenTexture,
	},
	io::{gfx::GfxContext, viewport::Viewport},
	scene::{SceneRenderHandler, SceneUpdateHandler},
};

use super::gfx::{
	mesh::store::QuadMeshLayer,
	voxel::{
		mesh::{BlockDescriptorVisual, VoxelWorldMesh},
		pipeline::{VoxelRenderingPipelineDesc, VoxelUniforms},
	},
};

#[derive(Debug)]
pub struct GameSceneState {
	engine: Entity,

	// World state
	voxel_uniforms: VoxelUniforms,
	main_viewport: Entity,

	// Block registry
	block_registry: MaterialRegistry,
	block_atlas: AtlasTexture,
	block_atlas_gfx: AtlasTextureGfx,
}

impl GameSceneState {
	fn new(engine: Entity, main_viewport: Entity) -> Self {
		// Acquire services
		let gfx = &*engine.get::<GfxContext>();
		let mut asset_mgr = engine.get_mut::<AssetManager>();

		// Create block registry
		let block_atlas = AtlasTexture::new(UVec2::new(100, 100), UVec2::new(16, 16));
		let mut block_registry = MaterialRegistry::default();

		block_registry.register(
			"crucible:air",
			OwnedEntity::new().with(MaterialDescriptorBase::default()),
		);

		// Create voxel uniforms
		let block_atlas_gfx = AtlasTextureGfx::new(gfx, &block_atlas, Some("block atlas"));
		let voxel_uniforms = VoxelUniforms::new(gfx, &mut asset_mgr, &block_atlas_gfx.view);

		// Create state
		let mut state = Self {
			engine,
			voxel_uniforms,
			main_viewport,
			block_registry,
			block_atlas,
			block_atlas_gfx,
		};

		// Load default materials
		state.create_material(
			"crucible_prototyping:one".to_string(),
			&image::load_from_memory(include_bytes!("gfx/res/placeholder_material_1.png"))
				.unwrap_pretty()
				.into_rgba32f(),
		);

		state.create_material(
			"crucible_prototyping:two".to_string(),
			&image::load_from_memory(include_bytes!("gfx/res/placeholder_material_2.png"))
				.unwrap_pretty()
				.into_rgba32f(),
		);

		state.create_material(
			"crucible_prototyping:three".to_string(),
			&image::load_from_memory(include_bytes!("gfx/res/placeholder_material_3.png"))
				.unwrap_pretty()
				.into_rgba32f(),
		);

		// Create slabs
		{
			// Load common slab texture
			let atlas_tile = state.block_atlas.add(
				&image::load_from_memory(include_bytes!("gfx/res/placeholder_material_3.png"))
					.unwrap_pretty()
					.into_rgba32f(),
			);

			// Create slabs
			state.register_material(
				"crucible_prototyping:four".to_string(),
				OwnedEntity::new()
					.with(MaterialDescriptorBase::default())
					.with(BlockDescriptorVisual::Mesh {
						mesh: QuadMeshLayer::default().with_cube(
							Vec3::ZERO,
							Vec3::new(1.0, 0.5, 1.0),
							atlas_tile,
						),
					}),
			);

			state.register_material(
				"crucible_prototyping:five".to_string(),
				OwnedEntity::new()
					.with(MaterialDescriptorBase::default())
					.with(BlockDescriptorVisual::Mesh {
						mesh: QuadMeshLayer::default().with_cube(
							Vec3::new(0.0, 0.5, 0.0),
							Vec3::new(1.0, 0.5, 1.0),
							atlas_tile,
						),
					}),
			);

			state.register_material(
				"crucible_prototyping:six".to_string(),
				OwnedEntity::new()
					.with(MaterialDescriptorBase::default())
					.with(BlockDescriptorVisual::Mesh {
						mesh: QuadMeshLayer::default().with_cube(
							Vec3::ZERO,
							Vec3::new(0.5, 1.0, 1.0),
							atlas_tile,
						),
					}),
			);
		}

		state.upload_atlases(gfx);

		// Return state
		state
	}

	pub fn create_material(&mut self, id: String, texture: &Rgba32FImage) {
		let atlas_tile = self.block_atlas.add(texture);

		self.register_material(
			id,
			OwnedEntity::new()
				.with(MaterialDescriptorBase::default())
				.with(BlockDescriptorVisual::cubic_simple(atlas_tile)),
		);
	}

	pub fn register_material(&mut self, id: String, descriptor: OwnedEntity) {
		self.block_registry.register(id, descriptor);
	}

	pub fn upload_atlases(&mut self, gfx: &GfxContext) {
		self.block_atlas_gfx.update(gfx, &self.block_atlas);
	}

	pub fn update(&mut self, me: Entity) {
		// Decompose self
		let gfx = &*self.engine.get::<GfxContext>();
		let mut world_data = me.get_mut::<VoxelWorldData>();
		let mut world_mesh = me.get_mut::<VoxelWorldMesh>();

		// Update chunk meshes
		for chunk in world_data.flush_flagged() {
			world_mesh.flag_chunk(chunk);

			// TODO: Make this more conservative
			let chunk_data = chunk.get::<VoxelChunkData>();

			for face in BlockFace::variants() {
				let Some(neighbor) = chunk_data.neighbor(face) else {
				continue;
				};

				world_mesh.flag_chunk(neighbor);
			}
		}

		world_mesh.update_chunks(gfx, &self.block_atlas, &self.block_registry, None);
	}

	pub fn render(&mut self, me: Entity, frame: &mut wgpu::SurfaceTexture) {
		// Acquire services
		let gfx = &*self.engine.get::<GfxContext>();
		let mut asset_mgr = self.engine.get_mut::<AssetManager>();
		let world_mesh = me.get::<VoxelWorldMesh>();

		// Acquire viewport
		let viewport = self.main_viewport.get::<Viewport>();
		let mut depth_texture = self.main_viewport.get_mut::<FullScreenTexture>();
		let depth_texture_format = depth_texture.wgpu_descriptor().format;
		let (_, depth_texture_view) = depth_texture.acquire(gfx, &viewport).unwrap();

		// Create encoder
		let view = frame
			.texture
			.create_view(&wgpu::TextureViewDescriptor::default());

		let mut encoder = gfx
			.device
			.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

		// Encode rendering commands
		{
			let pipeline = asset_mgr.load(
				&VoxelRenderingPipelineDesc {
					surface_format: viewport.curr_config().format,
					depth_format: depth_texture_format,
					is_wireframe: false,
					back_face_culling: true,
				},
				gfx,
			);

			let chunk_pass = world_mesh.prepare_chunk_draw_pass();

			// Begin render pass
			let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
				label: Some("main render pass"),
				color_attachments: &[Some(wgpu::RenderPassColorAttachment {
					view: &view,
					ops: wgpu::Operations {
						load: wgpu::LoadOp::Clear(wgpu::Color {
							r: 0.45,
							g: 0.66,
							b: 1.0,
							a: 1.0,
						}),
						store: true,
					},
					resolve_target: None,
				})],
				depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
					view: &*depth_texture_view,
					depth_ops: Some(wgpu::Operations {
						load: wgpu::LoadOp::Clear(f32::INFINITY),
						store: true,
					}),
					stencil_ops: None,
				}),
			});

			// Setup pipeline
			{
				pass.set_pipeline(&pipeline);

				// TODO
				// 				let aspect = viewport.curr_surface_aspect().unwrap();
				// 				// FIXME: Why does "_lh" correspond to a right-handed coordinate system?!
				// 				let proj = Mat4::perspective_lh(70f32.to_radians(), aspect, 0.1, 100.);
				// 				let view = self.free_cam.view_matrix();
				// 				let full = proj * view;
				//
				// 				self.voxel_uniforms.set_camera_matrix(gfx, full);
			}

			// Render world
			chunk_pass.push(&self.voxel_uniforms, &mut pass);
		}

		// Submit work
		gfx.queue.submit([encoder.finish()]);
	}
}

pub fn make_game_scene(engine: Entity, main_viewport: Entity) -> OwnedEntity {
	OwnedEntity::new()
		.with(GameSceneState::new(engine, main_viewport))
		.with(VoxelWorldData::default())
		.with(VoxelWorldMesh::default())
		.with(SceneUpdateHandler::new(move |me| {
			me.get_mut::<GameSceneState>().update(me);
		}))
		.with(SceneRenderHandler::new(move |me, frame| {
			me.get_mut::<GameSceneState>().render(me, frame);
		}))
}

fn create_chunk(pos: ChunkVec) -> OwnedEntity {
	OwnedEntity::new().with_debug_label(format_args!("chunk at {pos:?}"))
}
