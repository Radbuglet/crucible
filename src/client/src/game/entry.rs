use crucible_common::{
	game::material::{MaterialRegistry, MaterialStateBase},
	voxel::{
		coord::{EntityLocation, Location, RayCast},
		data::{BlockState, VoxelChunkData, VoxelWorldData},
		math::{BlockFace, ChunkVec, EntityVec, WorldVec},
	},
};
use crucible_util::{
	debug::{
		error::ResultExt,
		userdata::{BoxedUserdata, DebugOpaque, ErasedUserdata},
	},
	lang::{iter::VolumetricIter, polyfill::OptionPoly},
	mem::c_enum::CEnum,
};
use geode::prelude::*;
use image::Rgba32FImage;
use typed_glam::glam::{Mat4, UVec2};
use winit::{
	dpi::PhysicalPosition,
	event::{MouseButton, VirtualKeyCode},
	window::CursorGrabMode,
};

use crate::{
	engine::{
		assets::AssetManager,
		gfx::{
			atlas::{AtlasTexture, AtlasTextureGfx},
			texture::FullScreenTexture,
		},
		io::{gfx::GfxContext, input::InputManager, viewport::Viewport},
		scene::{SceneBundle, SceneRenderEvent, SceneUpdateEvent},
	},
	game::{player::camera::FreeCamInputs, voxel::pipeline::VoxelRenderingPipelineDesc},
};

use super::{
	player::camera::FreeCamController,
	voxel::{
		material::{
			BasicMaterialDescriptorBundle, InvisibleBlockDescriptorBundle, MaterialStateVisualBlock,
		},
		mesh::{VoxelChunkMesh, VoxelWorldMesh},
		pipeline::VoxelUniforms,
	},
};

// === PlaySceneState === //

#[derive(Debug)]
pub struct PlaySceneState {
	// Camera controller
	has_control: bool,
	free_cam: FreeCamController,

	// World state
	world_data: VoxelWorldData,
	world_mesh: VoxelWorldMesh,
	voxel_uniforms: VoxelUniforms,
	main_viewport: Entity,

	// Block registry
	block_atlas: AtlasTexture,
	block_atlas_gfx: AtlasTextureGfx,
	block_registry: MaterialRegistry,
	materials: Vec<Entity>,
	selected_material_idx: usize,
}

impl PlaySceneState {
	pub fn new(
		mut cx: (
			&Universe,
			&GfxContext,
			&mut AssetManager,
			&mut Storage<MaterialStateBase>,
			&mut Archetype<InvisibleBlockDescriptorBundle>,
		),
		main_viewport: Entity,
	) -> Self {
		// Acquire context
		decompose!(cx => cx & {
			gfx: &GfxContext,
			asset_mgr: &mut AssetManager,
			invisible_descriptor_arch: &mut Archetype<InvisibleBlockDescriptorBundle>,
		});

		// Create free cam
		let has_control = false;
		let free_cam = FreeCamController::default();

		// Create world state
		let world_data = VoxelWorldData::default();
		let world_mesh = VoxelWorldMesh::default();

		// Create block registry
		let block_atlas = AtlasTexture::new(UVec2::new(100, 100), UVec2::new(16, 16));
		let mut block_registry = MaterialRegistry::default();
		let materials = Vec::new();
		let selected_material_idx = 0;
		{
			let air_descriptor = invisible_descriptor_arch.spawn_with(
				decompose!(cx),
				"air descriptor",
				InvisibleBlockDescriptorBundle {
					base: MaterialStateBase::default(),
				},
			);
			block_registry.register(decompose!(cx), "crucible:air", air_descriptor);
		}

		// Create voxel uniforms
		let block_atlas_gfx = AtlasTextureGfx::new(gfx, &block_atlas, Some("block atlas"));
		let voxel_uniforms = VoxelUniforms::new((gfx, asset_mgr), &block_atlas_gfx.view);

		// Create scene
		Self {
			has_control,
			free_cam,
			world_data,
			world_mesh,
			voxel_uniforms,
			main_viewport,
			block_atlas,
			block_atlas_gfx,
			block_registry,
			materials,
			selected_material_idx,
		}
	}

	pub fn make_bundle(self) -> SceneBundle {
		SceneBundle {
			userdata: Box::new(self),
			update_handler: DebugOpaque::new(Self::on_update),
			render_handler: DebugOpaque::new(Self::on_render),
		}
	}

	pub fn create_material(
		&mut self,
		mut cx: (
			&mut Archetype<BasicMaterialDescriptorBundle>,
			&mut Storage<MaterialStateBase>,
			&mut Storage<MaterialStateVisualBlock>,
		),
		id: String,
		texture: &Rgba32FImage,
	) {
		decompose!(cx => cx & { material_bundle: &mut Archetype<BasicMaterialDescriptorBundle> });

		// Place into atlas
		let atlas_tile = self.block_atlas.add(texture);

		// Spawn material descriptor
		let descriptor = material_bundle.spawn_with(
			decompose!(cx),
			format_args!("{id} block descriptor"),
			BasicMaterialDescriptorBundle {
				base: MaterialStateBase::default(),
				visual: MaterialStateVisualBlock { atlas_tile },
			},
		);

		// Register material
		self.block_registry.register(decompose!(cx), id, descriptor);

		self.materials.push(descriptor);
	}

	pub fn create_default_materials(
		&mut self,
		mut cx: (
			&mut Archetype<BasicMaterialDescriptorBundle>,
			&mut Storage<MaterialStateBase>,
			&mut Storage<MaterialStateVisualBlock>,
		),
	) {
		self.create_material(
			decompose!(cx),
			"crucible_prototyping:one".to_string(),
			&image::load_from_memory(include_bytes!("voxel/textures/placeholder_material_1.png"))
				.unwrap_pretty()
				.into_rgba32f(),
		);
		self.create_material(
			decompose!(cx),
			"crucible_prototyping:two".to_string(),
			&image::load_from_memory(include_bytes!("voxel/textures/placeholder_material_2.png"))
				.unwrap_pretty()
				.into_rgba32f(),
		);
		self.create_material(
			decompose!(cx),
			"crucible_prototyping:three".to_string(),
			&image::load_from_memory(include_bytes!("voxel/textures/placeholder_material_3.png"))
				.unwrap_pretty()
				.into_rgba32f(),
		);
	}

	pub fn upload_atlases(&mut self, (gfx,): (&GfxContext,)) {
		self.block_atlas_gfx.update(gfx, &self.block_atlas);
	}

	fn on_update(dyn_cx: &Provider, me: Entity, _event: SceneUpdateEvent) {
		// Extract context
		unpack!(dyn_cx => {
			gfx: &GfxContext,
			chunk_arch: @arch ChunkBundle,
			userdatas: @mut Storage<BoxedUserdata>,
			viewports: @ref Storage<Viewport>,
			input_managers: @ref Storage<InputManager>,
			chunk_datas: @mut Storage<VoxelChunkData>,
			chunk_meshes: @mut Storage<VoxelChunkMesh>,
			descriptor_base_states: @ref Storage<MaterialStateBase>,
			descriptor_visual_states: @ref Storage<MaterialStateVisualBlock>,
		});

		let me = userdatas[me].downcast_mut::<Self>();

		// Handle interactions
		{
			let window = &viewports[me.main_viewport].window();
			let input_mgr = &input_managers[me.main_viewport];

			if !input_mgr.has_focus() {
				me.has_control = false;
			}

			let esc_pressed = input_mgr.key(VirtualKeyCode::Escape).recently_pressed();
			let left_pressed = input_mgr.button(MouseButton::Left).recently_pressed();
			let right_pressed = input_mgr.button(MouseButton::Right).recently_pressed();

			if me.has_control {
				// Update camera
				me.free_cam.handle_mouse_move(input_mgr.mouse_delta());

				me.free_cam.process(
					(&me.world_data, chunk_datas),
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
				{
					let keys = [
						VirtualKeyCode::Key1,
						VirtualKeyCode::Key2,
						VirtualKeyCode::Key3,
						VirtualKeyCode::Key4,
						VirtualKeyCode::Key5,
					];
					for (i, &key) in keys[0..me.materials.len()].iter().enumerate() {
						if input_mgr.key(key).recently_pressed() {
							me.selected_material_idx = i;
							break;
						}
					}
				}

				// Handle chunk filling
				if input_mgr.key(VirtualKeyCode::Space).state() {
					// Determine camera position
					let pos = me.free_cam.pos();
					let pos = WorldVec::cast_from(pos.floor());
					let pos = Location::new(&me.world_data, pos);

					// Fill volume
					let set_state_cx =
						Provider::new_with_parent_and_comps(dyn_cx, (&mut **chunk_arch,));

					for [x, y, z] in VolumetricIter::new([6, 6, 6]) {
						let [x, y, z] = [x as i32 - 3, y as i32 - 10, z as i32 - 3];

						pos.at_relative((&me.world_data, chunk_datas), WorldVec::new(x, y, z))
							.set_state_or_create(
								(&mut me.world_data, chunk_datas),
								&set_state_cx,
								Self::chunk_factory,
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
						EntityLocation::new(&me.world_data, me.free_cam.pos()),
						EntityVec::from_glam(me.free_cam.facing().as_dvec3()),
					);

					let cx = (&me.world_data, &**chunk_datas);

					for mut isect in ray.step_for(cx, 6.) {
						if isect
							.block
							.state(cx)
							.p_is_some_and(|state| state.material != 0)
						{
							let mut target = isect.block.at_neighbor(cx, isect.face.invert());
							let material = me.materials[me.selected_material_idx];
							let material = descriptor_base_states[material].slot();

							target.set_state_or_create(
								(&mut me.world_data, chunk_datas),
								&Provider::new_with_parent_and_comps(dyn_cx, (&mut **chunk_arch,)),
								Self::chunk_factory,
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
						me.free_cam.pos(),
						EntityVec::from_glam(me.free_cam.facing().as_dvec3()),
					);

					let cx = (&me.world_data, &**chunk_datas);

					for mut isect in ray.step_for(cx, 6.) {
						if isect
							.block
							.state(cx)
							.p_is_some_and(|state| state.material != 0)
						{
							isect.block.set_state_or_create(
								(&mut me.world_data, chunk_datas),
								&Provider::new_with_parent_and_comps(dyn_cx, (&mut **chunk_arch,)),
								Self::chunk_factory,
								BlockState::default(),
							);
							break;
						}
					}
				}

				// Handle control release
				if esc_pressed {
					me.has_control = false;

					window.set_cursor_visible(true);
					window.set_cursor_grab(CursorGrabMode::None).log();
				}
			} else {
				// Handle control acquire
				if left_pressed || right_pressed {
					me.has_control = true;

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
		}

		// Update chunk meshes
		for chunk in me.world_data.flush_flagged((chunk_datas,)) {
			me.world_mesh.flag_chunk((chunk_meshes,), chunk);

			// TODO: Make this more conservative
			let chunk_data = &chunk_datas[chunk];

			for face in BlockFace::variants() {
				let Some(neighbor) = chunk_data.neighbor(face) else {
					continue;
				};

				me.world_mesh.flag_chunk((chunk_meshes,), neighbor);
			}
		}

		me.world_mesh.update_chunks(
			(
				gfx,
				&me.block_atlas,
				&me.block_registry,
				chunk_datas,
				chunk_meshes,
				descriptor_visual_states,
			),
			None,
		);
	}

	fn on_render(cx: &Provider, me: Entity, event: SceneRenderEvent) {
		// Extract context
		unpack!(cx => {
			gfx: &GfxContext,
			asset_mgr: &mut AssetManager,
			scene_userdatas: @mut Storage<BoxedUserdata>,
			depth_textures: @mut Storage<FullScreenTexture>,
			viewports: @ref Storage<Viewport>,
			chunk_meshes: @mut Storage<VoxelChunkMesh>,
		});

		let me = scene_userdatas[me].downcast_mut::<Self>();
		let frame = event.frame;

		// Acquire viewport and depth texture
		let viewport = &viewports[me.main_viewport];
		let depth_texture = &mut depth_textures[me.main_viewport];
		let depth_texture_format = depth_texture.wgpu_descriptor().format;
		let (_, depth_texture_view) = depth_texture.acquire((gfx, viewport)).unwrap();

		// Create encoder
		let view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
			label: Some("frame view"),
			format: None,
			dimension: None,
			aspect: wgpu::TextureAspect::All,
			base_mip_level: 0,
			mip_level_count: None,
			base_array_layer: 0,
			array_layer_count: None,
		});

		let mut encoder = gfx
			.device
			.create_command_encoder(&wgpu::CommandEncoderDescriptor {
				label: Some("main viewport renderer"),
			});

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

				let aspect = viewport.curr_surface_aspect().unwrap();
				let proj = Mat4::perspective_lh(70f32.to_radians(), aspect, 0.1, 100.);
				let view = me.free_cam.view_matrix();
				let full = proj * view;

				me.voxel_uniforms.set_camera_matrix(gfx, full);
			}

			// Render world
			me.world_mesh
				.render_chunks((chunk_meshes, &me.voxel_uniforms), &mut pass);
		}

		// Submit work
		gfx.queue.submit([encoder.finish()]);
	}

	fn chunk_factory(cx: &Provider, pos: ChunkVec) -> Entity {
		unpack!(cx => {
			arch_chunk: @arch ChunkBundle,
		});

		let chunk = arch_chunk.spawn(format_args!("chunk at {pos:?}"));
		log::info!("Spawning chunk at {pos:?}: {chunk:?}");
		chunk
	}
}

// === ChunkBundle === //

bundle! {
	#[derive(Debug)]
	pub struct ChunkBundle {
		pub data: VoxelChunkData,
		pub mesh: VoxelChunkMesh,
	}
}

impl BuildableArchetypeBundle for ChunkBundle {
	fn create_archetype(universe: &Universe) -> ArchetypeHandle<Self> {
		universe.create_archetype("ChunkBundle")
	}
}
