use bort::{Entity, OwnedEntity};
use crucible_common::{
	actor::{kinds::spatial::update_kinematic_spatials, manager::ActorManager},
	material::{MaterialDescriptorBase, MaterialRegistry},
	world::{
		data::{BlockLocation, BlockState, VoxelChunkFactory, VoxelWorldData},
		math::{Aabb3, BlockFace, WorldVec},
	},
};
use crucible_util::{debug::error::ResultExt, mem::c_enum::CEnum};
use image::Rgba32FImage;
use typed_glam::glam::UVec2;

use crate::engine::{
	assets::AssetManager,
	gfx::{
		atlas::{AtlasTexture, AtlasTextureGfx},
		camera::CameraManager,
		texture::FullScreenTexture,
	},
	io::{gfx::GfxContext, viewport::Viewport},
	scene::{SceneRenderHandler, SceneUpdateHandler},
};

use super::{
	actors::player::{
		reset_kinematic_accelerations_to_gravity, spawn_local_player, PlayerInputController,
	},
	gfx::voxel::{
		mesh::{BlockDescriptorVisual, VoxelWorldMesh},
		pipeline::{load_opaque_block_pipeline, VoxelUniforms},
	},
};

// === Factories === //

pub fn make_game_scene(engine: Entity, main_viewport: Entity) -> OwnedEntity {
	// Construct the scene
	let scene = OwnedEntity::new()
		.with(GameSceneState::new(engine, main_viewport))
		.with(CameraManager::default())
		.with(ActorManager::default())
		.with(PlayerInputController::default())
		.with(VoxelWorldData::new(VoxelChunkFactory::new(|pos| {
			OwnedEntity::new().with_debug_label(format_args!("chunk at {pos:?}"))
		})))
		.with(VoxelWorldMesh::default())
		.with(SceneUpdateHandler::new(|me| {
			me.get_mut::<GameSceneState>().update(me);
		}))
		.with(SceneRenderHandler::new(|me, frame| {
			me.get_mut::<GameSceneState>().render(me, frame);
		}));

	// Register materials
	let mut scene_state = scene.get_mut::<GameSceneState>();
	let material = scene_state.register_block_material(
		"crucible_prototyping:one".to_string(),
		&image::load_from_memory(include_bytes!(
			"gfx/res/textures/placeholder_material_1.png"
		))
		.unwrap_pretty()
		.into_rgba32f(),
	);
	scene_state.upload_atlases(&engine.get::<GfxContext>());

	// Spawn local player
	let mut actors = scene.get_mut::<ActorManager>();
	let mut player_inputs = scene.get_mut::<PlayerInputController>();
	let local_player = spawn_local_player(&mut actors);
	player_inputs.set_local_player(Some(local_player));

	// Populate world
	let mut world_data = scene.get_mut::<VoxelWorldData>();
	let the_box = Aabb3 {
		origin: WorldVec::splat(-5),
		size: WorldVec::splat(10),
	};
	for face in BlockFace::variants() {
		if face == BlockFace::PositiveY {
			continue;
		}

		for pos in the_box.quad(face).extrude_hv(1).iter_blocks() {
			BlockLocation::new(&world_data, pos).set_state_or_create(
				&mut world_data,
				BlockState {
					material,
					..Default::default()
				},
			);
		}
	}

	scene
}

// === Components === //

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
		let assets = &mut *engine.get_mut::<AssetManager>();

		// Create block registry
		let block_atlas = AtlasTexture::new(UVec2::new(100, 100), UVec2::new(16, 16));
		let mut block_registry = MaterialRegistry::default();

		block_registry.register(
			"crucible:air",
			OwnedEntity::new().with(MaterialDescriptorBase::default()),
		);

		// Create voxel uniforms
		let block_atlas_gfx = AtlasTextureGfx::new(gfx, &block_atlas, Some("block atlas"));
		let voxel_uniforms = VoxelUniforms::new(assets, gfx, &block_atlas_gfx.view);

		// Create state
		Self {
			engine,
			voxel_uniforms,
			main_viewport,
			block_registry,
			block_atlas,
			block_atlas_gfx,
		}
	}

	pub fn register_block_material(&mut self, id: String, texture: &Rgba32FImage) -> u16 {
		let atlas_tile = self.block_atlas.add(texture);

		self.register_material(
			id,
			OwnedEntity::new()
				.with(MaterialDescriptorBase::default())
				.with(BlockDescriptorVisual::cubic_simple(atlas_tile)),
		)
	}

	pub fn register_material(&mut self, id: String, descriptor: OwnedEntity) -> u16 {
		self.block_registry.register(id, descriptor)
	}

	pub fn upload_atlases(&mut self, gfx: &GfxContext) {
		self.block_atlas_gfx.update(gfx, &self.block_atlas);
	}

	pub fn update(&mut self, me: Entity) {
		// Decompose self
		let gfx = &*self.engine.get::<GfxContext>();
		let actors = me.get::<ActorManager>();
		let mut world_data = me.get_mut::<VoxelWorldData>();
		let mut world_mesh = me.get_mut::<VoxelWorldMesh>();
		let mut player_inputs = me.get_mut::<PlayerInputController>();
		let mut camera = me.get_mut::<CameraManager>();

		// Reset camera
		camera.unset();

		// Process actors
		reset_kinematic_accelerations_to_gravity(&actors);
		player_inputs.update(self.main_viewport, &mut camera, &mut world_data);
		update_kinematic_spatials(&actors, &world_data, 1.0 / 60.0);

		// Update chunk meshes
		for chunk in world_data.flush_flagged() {
			world_mesh.flag_chunk(chunk);

			// TODO: Make this more conservative
			let chunk_data = world_data.chunk_state(chunk);

			for face in BlockFace::variants() {
				let Some(neighbor) = chunk_data.neighbor(face) else {
				continue;
				};

				world_mesh.flag_chunk(neighbor);
			}
		}

		world_mesh.update_chunks(
			&world_data,
			gfx,
			&self.block_atlas,
			&self.block_registry,
			None,
		);
	}

	pub fn render(&mut self, me: Entity, frame: &mut wgpu::SurfaceTexture) {
		// Acquire services
		let gfx = &*self.engine.get::<GfxContext>();
		let mut assets = self.engine.get_mut::<AssetManager>();
		let world_mesh = me.get::<VoxelWorldMesh>();
		let camera = me.get::<CameraManager>();

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
			let pipeline = load_opaque_block_pipeline(
				&mut assets,
				gfx,
				viewport.curr_config().format,
				depth_texture_format,
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
				pipeline.bind_pipeline(&mut pass);

				let aspect = viewport.curr_surface_aspect().unwrap();
				let xform = camera.get_camera_xform(aspect);
				self.voxel_uniforms.set_camera_matrix(gfx, xform);
			}

			// Render world
			chunk_pass.push(&self.voxel_uniforms, &mut pass);
		}

		// Submit work
		gfx.queue.submit([encoder.finish()]);
	}
}
