use crucible_common::voxel::{
	coord::{EntityLocation, Location, RayCast},
	data::{BlockState, VoxelChunkData, VoxelWorldData},
	math::{BlockFace, ChunkVec, EntityVec, WorldVec},
};
use crucible_core::{
	debug::{
		error::ResultExt,
		userdata::{BoxedUserdata, DebugOpaque},
	},
	ecs::{
		context::{decompose, unpack, Provider},
		entity::{ArchetypeId, Entity},
		storage::CelledStorage,
		storage::Storage,
		universe::{ArchetypeHandle, Res, ResRw, Universe, UniverseResource},
	},
	lang::{explicitly_bind::ExplicitlyBind, iter::VolumetricIter, polyfill::OptionPoly},
	mem::c_enum::CEnum,
};
use typed_glam::glam::Mat4;
use wgpu::util::DeviceExt;
use winit::{
	dpi::PhysicalPosition,
	event::{MouseButton, VirtualKeyCode},
	window::CursorGrabMode,
};

use crate::{
	engine::{
		gfx::texture::FullScreenTexture,
		io::{gfx::GfxContext, input::InputManager, viewport::Viewport},
		resources::ResourceManager,
		scene::{
			SceneArch, SceneRenderEvent, SceneRenderHandler, SceneUpdateEvent, SceneUpdateHandler,
		},
	},
	game::{player::camera::FreeCamInputs, voxel::pipeline::VoxelRenderingPipelineDesc},
};

use super::{
	player::camera::FreeCamController,
	voxel::{
		mesh::{VoxelChunkMesh, VoxelWorldMesh},
		pipeline::VoxelUniforms,
	},
};

// === PlaySceneState === //

#[derive(Debug, Default)]
pub struct PlaySceneState {
	has_control: bool,
	free_cam: FreeCamController,
	world_data: VoxelWorldData,
	world_mesh: VoxelWorldMesh,
	main_viewport: ExplicitlyBind<Entity>,
	voxel_uniforms: ExplicitlyBind<VoxelUniforms>,
	time: f64,
}

impl PlaySceneState {
	pub fn spawn(
		mut cx: (
			&Universe,
			&GfxContext,
			&SceneArch,
			&mut Storage<BoxedUserdata>,
			&mut Storage<SceneUpdateHandler>,
			&mut Storage<SceneRenderHandler>,
			&mut ResourceManager,
		),
		main_viewport: Entity,
	) -> Entity {
		decompose!(cx => cx & {
			gfx: &GfxContext,
			res_mgr: &mut ResourceManager,
			scene_arch: &SceneArch,
		});

		// Create block texture
		let block_image = image::load_from_memory(include_bytes!(
			"./voxel/textures/placeholder_material_1.png"
		))
		.unwrap();

		let block_texture = gfx.device.create_texture_with_data(
			&gfx.queue,
			&wgpu::TextureDescriptor {
				label: Some("block :)"),
				size: wgpu::Extent3d {
					width: block_image.width(),
					height: block_image.height(),
					depth_or_array_layers: 1,
				},
				mip_level_count: 1,
				sample_count: 1,
				dimension: wgpu::TextureDimension::D2,
				format: wgpu::TextureFormat::Rgba32Float,
				usage: wgpu::TextureUsages::TEXTURE_BINDING,
			},
			bytemuck::cast_slice(&block_image.into_rgba32f()),
		);
		let block_texture_view = block_texture.create_view(&wgpu::TextureViewDescriptor::default());

		// Create scene state
		let mut scene_state = Box::new(Self::default());
		ExplicitlyBind::bind(&mut scene_state.main_viewport, main_viewport);
		ExplicitlyBind::bind(
			&mut scene_state.voxel_uniforms,
			VoxelUniforms::new((gfx, res_mgr), &block_texture_view),
		);

		// Create scene entity
		scene_arch.spawn(
			decompose!(cx),
			scene_state,
			DebugOpaque::new(Self::on_update),
			DebugOpaque::new(Self::on_render),
		)
	}

	fn on_update(dyn_cx: &Provider, me: Entity, _event: SceneUpdateEvent) {
		// Extract context
		unpack!(dyn_cx => {
			gfx: &GfxContext,
			chunk_arch: Res<&ChunkArchetype>,
			userdatas: ResRw<&mut Storage<BoxedUserdata>>,
			viewports: ResRw<&Storage<Viewport>>,
			input_managers: ResRw<&Storage<InputManager>>,
			chunk_datas: ResRw<&mut CelledStorage<VoxelChunkData>>,
			chunk_meshes: ResRw<&mut Storage<VoxelChunkMesh>>,
		});

		let me = userdatas.get_downcast_mut::<Self>(me);

		// Update timer
		me.time += 0.1;

		// Handle interactions
		{
			let window = &viewports[*me.main_viewport].window();
			let input_mgr = &input_managers[*me.main_viewport];

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
					(&me.world_data, &*chunk_datas.as_celled_view()),
					FreeCamInputs {
						up: input_mgr.key(VirtualKeyCode::E).state(),
						down: input_mgr.key(VirtualKeyCode::Q).state(),
						left: input_mgr.key(VirtualKeyCode::A).state(),
						right: input_mgr.key(VirtualKeyCode::D).state(),
						fore: input_mgr.key(VirtualKeyCode::W).state(),
						back: input_mgr.key(VirtualKeyCode::S).state(),
					},
				);

				// Handle chunk filling
				if input_mgr.key(VirtualKeyCode::Space).state() {
					// Determine camera position
					let pos = me.free_cam.pos();
					let pos = WorldVec::cast_from(pos.floor());
					let pos = Location::new(&me.world_data, pos);

					// Fill volume
					let set_state_cx = Provider::new_with_parent(Some(dyn_cx)).with(chunk_arch);

					for [x, y, z] in VolumetricIter::new([6, 6, 6]) {
						let [x, y, z] = [x as i32 - 3, y as i32 - 10, z as i32 - 3];

						pos.at_relative(
							(&me.world_data, chunk_datas.as_celled_view()),
							WorldVec::new(x, y, z),
						)
						.set_state_or_create(
							(&mut me.world_data, chunk_datas, &set_state_cx),
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

					let cx = (&me.world_data, &*chunk_datas.as_celled_view());

					for mut isect in ray.step_for(cx, 6.) {
						if isect
							.block
							.state(cx)
							.p_is_some_and(|state| state.material != 0)
						{
							let mut target = isect.block.at_neighbor(cx, isect.face.invert());
							target.set_state_or_create(
								(
									&mut me.world_data,
									chunk_datas,
									&Provider::new().with(chunk_arch),
								),
								Self::chunk_factory,
								BlockState {
									material: 1,
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

					let cx = (&me.world_data, &*chunk_datas.as_celled_view());

					for mut isect in ray.step_for(cx, 6.) {
						if isect
							.block
							.state(cx)
							.p_is_some_and(|state| state.material != 0)
						{
							isect.block.set_state_or_create(
								(
									&mut me.world_data,
									chunk_datas,
									&Provider::new().with(chunk_arch),
								),
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
			let chunk_data = chunk_datas.get(chunk);

			for face in BlockFace::variants() {
				let Some(neighbor) = chunk_data.neighbor(face) else {
					continue;
				};

				me.world_mesh.flag_chunk((chunk_meshes,), neighbor);
			}
		}

		me.world_mesh
			.update_chunks((&gfx, chunk_datas, chunk_meshes), None);
	}

	fn on_render(cx: &Provider, me: Entity, event: SceneRenderEvent) {
		// Extract context
		unpack!(cx => {
			gfx: &GfxContext,
			res_mgr: &mut ResourceManager,
			scene_userdatas: ResRw<&mut Storage<BoxedUserdata>>,
			depth_textures: ResRw<&mut Storage<FullScreenTexture>>,
			viewports: ResRw<&Storage<Viewport>>,
			chunk_meshes: ResRw<&mut Storage<VoxelChunkMesh>>,
		});

		let me = scene_userdatas.get_downcast_mut::<Self>(me);
		let frame = event.frame;

		// Acquire viewport and depth texture
		let viewport = &viewports[*me.main_viewport];
		let depth_texture = &mut depth_textures[*me.main_viewport];
		let depth_texture_format = depth_texture.wgpu_descriptor().format;
		let (_, depth_texture_view) = depth_texture.acquire((&gfx, viewport)).unwrap();

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
			let pipeline = res_mgr.load(
				&VoxelRenderingPipelineDesc {
					surface_format: viewport.curr_config().format,
					depth_format: depth_texture_format,
					is_wireframe: false,
					back_face_culling: true,
				},
				&gfx,
			);

			// Begin render pass
			let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
				label: Some("main render pass"),
				color_attachments: &[Some(wgpu::RenderPassColorAttachment {
					view: &view,
					ops: wgpu::Operations {
						load: wgpu::LoadOp::Clear(wgpu::Color {
							r: 0.5 + 0.5 * me.time.cos(),
							g: 0.1,
							b: 0.1,
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

				me.voxel_uniforms.set_camera_matrix(&gfx, full);
			}

			// Render world
			me.world_mesh
				.render_chunks((&chunk_meshes, &me.voxel_uniforms), &mut pass);
		}

		// Submit work
		gfx.queue.submit([encoder.finish()]);
	}

	fn chunk_factory(cx: &Provider, pos: ChunkVec) -> Entity {
		unpack!(cx => {
			universe: &Universe,
			arch_chunk: Res<&ChunkArchetype>,
		});

		let chunk = arch_chunk.spawn((universe,), pos);
		log::info!("Spawning chunk at {pos:?}");
		chunk
	}
}

// === ChunkArchetype === //

#[derive(Debug)]
pub struct ChunkArchetype(ArchetypeHandle);

impl UniverseResource for ChunkArchetype {
	fn create(universe: &Universe) -> Self {
		let arch = universe.create_archetype("chunk archetype");

		Self(arch)
	}
}

impl ChunkArchetype {
	pub fn id(&self) -> ArchetypeId {
		self.0.id()
	}

	pub fn spawn(&self, (universe,): (&Universe,), pos: ChunkVec) -> Entity {
		universe
			.archetype(self.id())
			.spawn(format_args!("chunk at {pos:?}"))
	}
}
