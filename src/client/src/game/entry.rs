use crucible_common::{
	game::material::{MaterialDescriptorBase, MaterialRegistry},
	voxel::{
		coord::{EntityLocation, Location, RayCast},
		data::{BlockState, VoxelChunkData, VoxelWorldData},
		math::{BlockFace, ChunkVec, EntityVec, WorldVec},
	},
};
use crucible_util::{
	debug::error::ResultExt,
	lang::{iter::VolumetricIter, polyfill::OptionPoly},
	mem::c_enum::CEnum,
};
use geode::{Entity, OwnedEntity};
use image::Rgba32FImage;
use typed_glam::glam::{Mat4, UVec2};
use winit::{
	dpi::PhysicalPosition,
	event::{MouseButton, VirtualKeyCode},
	window::CursorGrabMode,
};

use crate::engine::{
	assets::AssetManager,
	gfx::{
		atlas::{AtlasTexture, AtlasTextureGfx},
		texture::FullScreenTexture,
	},
	io::{gfx::GfxContext, input::InputManager, viewport::Viewport},
	scene::{SceneRenderHandler, SceneUpdateHandler},
};

use super::{
	player::camera::{FreeCamController, FreeCamInputs},
	voxel::{
		material::BlockDescriptorVisual,
		mesh::VoxelWorldMesh,
		pipeline::{VoxelRenderingPipelineDesc, VoxelUniforms},
	},
};

#[derive(Debug)]
pub struct GameSceneState {
	engine: Entity,
	time: f64,

	// Camera controller
	has_control: bool,
	free_cam: FreeCamController,

	// World state
	voxel_uniforms: VoxelUniforms,
	main_viewport: Entity,

	// Block registry
	block_registry: MaterialRegistry,
	block_atlas: AtlasTexture,
	block_atlas_gfx: AtlasTextureGfx,
	materials: Vec<Entity>,
	selected_material_idx: usize,
}

impl GameSceneState {
	fn new(engine: Entity, main_viewport: Entity) -> Self {
		// Acquire services
		let gfx = &*engine.get::<GfxContext>();
		let mut asset_mgr = engine.get_mut::<AssetManager>();

		// Create free cam
		let has_control = false;
		let free_cam = FreeCamController::default();

		// Create block registry
		let block_atlas = AtlasTexture::new(UVec2::new(100, 100), UVec2::new(16, 16));
		let mut block_registry = MaterialRegistry::default();
		let materials = Vec::new();

		let selected_material_idx = 0;
		block_registry.register(
			"crucible:air",
			Entity::new().with(MaterialDescriptorBase::default()),
		);

		// Create voxel uniforms
		let block_atlas_gfx = AtlasTextureGfx::new(gfx, &block_atlas, Some("block atlas"));
		let voxel_uniforms = VoxelUniforms::new(gfx, &mut asset_mgr, &block_atlas_gfx.view);

		// Create state
		let mut state = Self {
			engine,
			time: 0.,
			has_control,
			free_cam,
			voxel_uniforms,
			main_viewport,
			block_registry,
			block_atlas,
			block_atlas_gfx,
			materials,
			selected_material_idx,
		};

		// Load default materials
		state.create_material(
			"crucible_prototyping:one".to_string(),
			&image::load_from_memory(include_bytes!("voxel/textures/placeholder_material_1.png"))
				.unwrap_pretty()
				.into_rgba32f(),
		);

		state.create_material(
			"crucible_prototyping:two".to_string(),
			&image::load_from_memory(include_bytes!("voxel/textures/placeholder_material_2.png"))
				.unwrap_pretty()
				.into_rgba32f(),
		);

		state.create_material(
			"crucible_prototyping:three".to_string(),
			&image::load_from_memory(include_bytes!("voxel/textures/placeholder_material_3.png"))
				.unwrap_pretty()
				.into_rgba32f(),
		);

		state.upload_atlases(gfx);

		// Return state
		state
	}

	pub fn create_material(&mut self, id: String, texture: &Rgba32FImage) {
		// Place into atlas
		let atlas_tile = self.block_atlas.add(texture);

		// Spawn material descriptor
		let (descriptor, descriptor_ref) = Entity::new()
			.with(MaterialDescriptorBase::default())
			.with(BlockDescriptorVisual { atlas_tile })
			.split_guard();

		// Register material
		self.block_registry.register(id, descriptor);
		self.materials.push(descriptor_ref);
	}

	pub fn upload_atlases(&mut self, gfx: &GfxContext) {
		self.block_atlas_gfx.update(gfx, &self.block_atlas);
	}

	pub fn update(&mut self, me: Entity) {
		// Decompose self
		let gfx = &*self.engine.get::<GfxContext>();
		let mut world_data = me.get_mut::<VoxelWorldData>();
		let mut world_mesh = me.get_mut::<VoxelWorldMesh>();

		// Handle interactions
		{
			let viewport = self.main_viewport.get::<Viewport>();
			let window = viewport.window();
			let input_mgr = self.main_viewport.get::<InputManager>();

			if !input_mgr.has_focus() {
				self.has_control = false;
			}

			let esc_pressed = input_mgr.key(VirtualKeyCode::Escape).recently_pressed();
			let left_pressed = input_mgr.button(MouseButton::Left).recently_pressed();
			let right_pressed = input_mgr.button(MouseButton::Right).recently_pressed();

			if self.has_control {
				// Update camera
				self.free_cam.handle_mouse_move(input_mgr.mouse_delta());

				self.free_cam.process(
					&world_data,
					FreeCamInputs {
						up: input_mgr.key(VirtualKeyCode::E).state(),
						down: input_mgr.key(VirtualKeyCode::Q).state(),
						left: input_mgr.key(VirtualKeyCode::A).state(),
						right: input_mgr.key(VirtualKeyCode::D).state(),
						fore: input_mgr.key(VirtualKeyCode::W).state(),
						back: input_mgr.key(VirtualKeyCode::S).state(),
					},
				);

				// Process slot selection
				for (i, &key) in [
					VirtualKeyCode::Key1,
					VirtualKeyCode::Key2,
					VirtualKeyCode::Key3,
					VirtualKeyCode::Key4,
					VirtualKeyCode::Key5,
					VirtualKeyCode::Key6,
					VirtualKeyCode::Key7,
					VirtualKeyCode::Key8,
					VirtualKeyCode::Key9,
					VirtualKeyCode::Key0,
				][0..self.materials.len()]
					.iter()
					.enumerate()
				{
					if input_mgr.key(key).recently_pressed() {
						self.selected_material_idx = i;
						break;
					}
				}

				// Handle chunk filling
				if input_mgr.key(VirtualKeyCode::Space).state() {
					// Determine camera position
					let pos = self.free_cam.pos();
					let pos = WorldVec::cast_from(pos.floor());
					let pos = Location::new(&world_data, pos);

					// Fill volume
					for [x, y, z] in VolumetricIter::new([6, 6, 6]) {
						let [x, y, z] = [x as i32 - 3, y as i32 - 10, z as i32 - 3];

						pos.at_relative(&world_data, WorldVec::new(x, y, z))
							.set_state_or_create(
								&mut world_data,
								create_chunk,
								BlockState {
									material: 1,
									variant: 0,
									light_level: 255,
								},
							);
					}
				}

				if input_mgr.button(MouseButton::Right).recently_pressed() {
					let mut ray = RayCast::new_at(
						EntityLocation::new(&world_data, self.free_cam.pos()),
						EntityVec::from_glam(self.free_cam.facing().as_dvec3()),
					);

					for mut isect in ray.step_for(&world_data, 6.) {
						if isect
							.block
							.state(&world_data)
							.p_is_some_and(|state| state.material != 0)
						{
							let mut target =
								isect.block.at_neighbor(&world_data, isect.face.invert());

							let material = self.materials[self.selected_material_idx];
							let material = material.get::<MaterialDescriptorBase>().slot();

							target.set_state_or_create(
								&mut world_data,
								create_chunk,
								BlockState {
									material,
									variant: 0,
									light_level: 255,
								},
							);
							break;
						}
					}
				} else if input_mgr.button(MouseButton::Left).recently_pressed() {
					let mut ray = RayCast::new_uncached(
						self.free_cam.pos(),
						EntityVec::from_glam(self.free_cam.facing().as_dvec3()),
					);

					for mut isect in ray.step_for(&world_data, 6.) {
						if isect
							.block
							.state(&world_data)
							.p_is_some_and(|state| state.material != 0)
						{
							isect.block.set_state_or_create(
								&mut world_data,
								create_chunk,
								BlockState::default(),
							);
							break;
						}
					}
				}

				// Handle control release
				if esc_pressed {
					self.has_control = false;

					window.set_cursor_visible(true);
					window.set_cursor_grab(CursorGrabMode::None).log();
				}
			} else {
				// Handle control acquire
				if left_pressed || right_pressed {
					self.has_control = true;

					// Warp cursor to the center and lock it
					let win_sz = window.inner_size();
					let win_center = PhysicalPosition::new(win_sz.width / 2, win_sz.height / 2);
					window.set_cursor_position(win_center).log();

					let modes = [CursorGrabMode::Confined, CursorGrabMode::Locked];
					for mode in modes {
						if window.set_cursor_grab(mode).log().is_some() {
							break;
						}
					}

					// Hide cursor
					window.set_cursor_visible(false);
				}
			}

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
	}

	pub fn render(&mut self, me: Entity, frame: &mut wgpu::SurfaceTexture) {
		self.time += 0.1;

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
							r: 0.45 + 0.4 * self.time.cos(),
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

				let aspect = viewport.curr_surface_aspect().unwrap();
				let proj = Mat4::perspective_lh(70f32.to_radians(), aspect, 0.1, 100.);
				let view = self.free_cam.view_matrix();
				let full = proj * view;

				self.voxel_uniforms.set_camera_matrix(gfx, full);
			}

			// Render world
			chunk_pass.push(&self.voxel_uniforms, &mut pass);
		}

		// Submit work
		gfx.queue.submit([encoder.finish()]);
	}
}

pub fn make_game_scene(engine: Entity, main_viewport: Entity) -> OwnedEntity {
	Entity::new()
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

fn create_chunk(_pos: ChunkVec) -> OwnedEntity {
	Entity::new()
}
