#![allow(dead_code)]
#![feature(decl_macro)]
#![feature(duration_constants)]
#![feature(never_type)]

use crate::engine::context::GfxContext;
use crate::engine::input::InputTracker;
use crate::engine::run_loop::{
	start_run_loop, DepGuard, RunLoopCommand, RunLoopHandler, RunLoopStatTracker,
};
use crate::engine::util::camera::{GfxCameraManager, PerspectiveCamera};
use crate::engine::util::uniform::UniformManager;
use crate::engine::util::vec_ext::VecConvert;
use crate::engine::viewport::ViewportManager;
use crate::voxel::render::{VoxelRenderer, DEPTH_TEXTURE_FORMAT};
use anyhow::Context;
use cgmath::{Deg, InnerSpace, Matrix3, Rad, Vector2, Vector3, Zero};
use crucible_core::foundation::prelude::*;
use crucible_core::util::error::{AnyResult, ErrorFormatExt};
use crucible_core::util::meta_enum::EnumMeta;
use crucible_shared::voxel::coord::{Axis3, BlockPos, ChunkPos};
use crucible_shared::voxel::data::VoxelWorld;
use futures::executor::block_on;
use std::collections::VecDeque;
use std::f32::consts::PI;
use std::sync::Arc;
use std::time::Duration;
use winit::dpi::LogicalSize;
use winit::event::{DeviceEvent, DeviceId, MouseButton, VirtualKeyCode, WindowEvent};
use winit::event_loop::{EventLoop, EventLoopWindowTarget};
use winit::window::WindowBuilder;

pub mod engine;
pub mod voxel;

fn main() {
	if let Err(err) = main_inner() {
		eprintln!("{}", err.format_error(true));
	}
}

type Engine = Arc<
	MultiProvider<(
		// Foundational services
		Component<Executor>,
		Component<RwLockManager>,
		RwLockComponent<World>,
		// Core engine services
		LazyComponent<GfxContext>,
		RwLockComponent<ViewportManager>,
		RwLockComponent<InputTracker>,
		RwLockComponent<UniformManager>,
		LazyComponent<GfxCameraManager>,
		RwLockComponent<RunLoopStatTracker>,
		// Game services
		RwLockComponent<VoxelWorld>,
		RwLockComponent<VoxelRenderer>,
		RwLockComponent<GameState>,
	)>,
>;

struct GameState {
	camera: PerspectiveCamera,
	depth: Storage<DepthAttachment>,
	is_active: bool,
	main_window: Entity,
}

struct DepthAttachment {
	texture: wgpu::Texture,
	size: Vector2<u32>,
}

fn create_depth_texture(gfx: &GfxContext, size: Vector2<u32>) -> wgpu::Texture {
	gfx.device.create_texture(&wgpu::TextureDescriptor {
		label: Some("depth texture"),
		size: wgpu::Extent3d {
			width: size.x,
			height: size.y,
			depth_or_array_layers: 1,
		},
		mip_level_count: 1,
		sample_count: 1,
		dimension: wgpu::TextureDimension::D2,
		format: DEPTH_TEXTURE_FORMAT,
		usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
	})
}

fn main_inner() -> AnyResult<!> {
	// Initialize foundational services
	env_logger::init();

	let engine = Engine::default();
	let mut world = World::new();

	// Startup graphics singleton and create the main window
	log::info!("Initializing graphics subsystem...");
	log::info!("Creating EventLoop");
	let event_loop = EventLoop::new();
	let (gfx, main_window, vm) = {
		log::info!("Creating main window");
		let window = WindowBuilder::new()
			.with_title("Crucible")
			.with_visible(false)
			.with_inner_size(LogicalSize::new(1920_i32, 1080_i32))
			.with_min_inner_size(LogicalSize::new(100_i32, 100_i32))
			.build(&event_loop)
			.context("Failed to create main window.")?;

		log::info!("Initializing wgpu context");
		let (gfx, surface) = block_on(GfxContext::with_window(
			&window,
			wgpu::Features::POLYGON_MODE_LINE,
		))
		.context("Failed to initialize wgpu!")?;

		let mut vm = ViewportManager::new();
		let entity = world.spawn();
		vm.register_pair(&world, &gfx, entity, window, surface);

		(gfx, entity, vm)
	};
	log::info!("Done initializing graphics subsystem!");

	// Setup core engine services
	let input = InputTracker::new();
	let uniform = UniformManager::new(
		&gfx,
		Some("uniform manager"),
		wgpu::BufferUsages::UNIFORM,
		1024,
	);

	let camera = GfxCameraManager::new(&gfx);

	// Setup voxels
	let mut voxel_data = VoxelWorld::new();
	let mut voxel_render = VoxelRenderer::new(&gfx, &camera);

	for x in 0..32 {
		for y in 0..16 {
			for z in 0..32 {
				let ent_chunk = world.spawn();

				// Setup chunk data
				let chunk_pos = ChunkPos::new(x, y, z);
				voxel_data.add(&world, chunk_pos, ent_chunk);

				let data = voxel_data.get_chunk_mut(&world, ent_chunk).unwrap();
				data.set_block(BlockPos::new(0, 0, 0), 1);

				for (axis, _) in Axis3::values_iter() {
					if chunk_pos.raw[axis.vec_idx] == Vector3::new(31, 15, 31)[axis.vec_idx] {
						continue;
					}

					for d in 1..16 {
						data.set_block(BlockPos::new(0, 0, 0) + (axis.unit() * d), 1);
					}
				}

				// Mesh chunk
				voxel_render.mark_dirty(&world, ent_chunk);
			}
		}
	}

	// Start engine
	let mut depth = Storage::new();
	for e_win in vm.get_entities() {
		let viewport = vm.get_viewport(e_win).unwrap();
		viewport.window().set_visible(true);

		let size = viewport.window().inner_size().to_vec();
		let texture = create_depth_texture(&gfx, size);
		depth.insert(&world, e_win, DepthAttachment { texture, size });
	}

	engine.init_lock(world);
	engine.init_lock(uniform);
	engine.init_lock(input);
	engine.init(gfx);
	engine.init_lock(vm);
	engine.init_lock(voxel_data);
	engine.init_lock(voxel_render);
	engine.init(camera);

	engine.init_lock(GameState {
		camera: PerspectiveCamera {
			position: Vector3::new(0., 1., 10.),
			..Default::default()
		},
		depth,
		is_active: false,
		main_window,
	});
	engine.init_lock(RunLoopStatTracker::start(60));

	log::info!("Starting run loop!");
	start_run_loop(event_loop, engine, Handler);
}

struct Handler;

impl RunLoopHandler for Handler {
	type Engine = Engine;

	fn tick(
		&mut self,
		on_loop_ev: &mut VecDeque<RunLoopCommand>,
		engine: &Self::Engine,
		_event_loop: &EventLoopWindowTarget<()>,
		dep_guard: DepGuard,
	) {
		// Lock services
		let (wm, stats) = dep_guard.get();

		get_many!(&**engine, gfx: &GfxContext);
		lock_many_now!(
			engine.get_many() => _guard,
			world: &World,
			input: &mut InputTracker,
			state: &mut GameState,
			voxel_data: &VoxelWorld,
			voxel_render: &mut VoxelRenderer,
		);

		// Update chunks
		voxel_render.update_dirty(world, voxel_data, gfx, Duration::from_millis(10));

		// Process inputs
		{
			let mut is_active_dirty = false;

			if input.button(MouseButton::Left).recently_pressed() {
				state.is_active = true;
				is_active_dirty = true;
			}

			if input.key(VirtualKeyCode::Escape).recently_pressed() {
				if state.is_active {
					state.is_active = false;
					is_active_dirty = true;
				} else {
					on_loop_ev.fire(RunLoopCommand::Shutdown);
				}
			}

			if is_active_dirty {
				let window = wm.get_viewport(state.main_window).unwrap().window();
				let _ = window.set_cursor_grab(state.is_active);
				window.set_cursor_visible(!state.is_active);
			}
		}

		if state.is_active {
			let camera = &mut state.camera;

			// Calculate heading
			let mut heading = Vector3::zero();

			if input.key(VirtualKeyCode::W).state() {
				heading -= Vector3::unit_z();
			}

			if input.key(VirtualKeyCode::S).state() {
				heading += Vector3::unit_z();
			}

			if input.key(VirtualKeyCode::D).state() {
				heading += Vector3::unit_x();
			}

			if input.key(VirtualKeyCode::A).state() {
				heading -= Vector3::unit_x();
			}

			if input.key(VirtualKeyCode::Q).state() {
				heading -= Vector3::unit_y();
			}

			if input.key(VirtualKeyCode::E).state() {
				heading += Vector3::unit_y();
			}

			let heading = if heading.is_zero() {
				heading
			} else {
				heading.normalize()
			};

			let speed = if input.key(VirtualKeyCode::LShift).state() {
				5.
			} else {
				50.
			};

			// Rotate camera
			let rel = -input.mouse_delta() * 0.3;
			camera.yaw += Deg(rel.x as _).into();
			camera.yaw %= Deg(360.).into();
			camera.pitch += Deg(rel.y as _).into();
			camera.pitch = Rad(camera.pitch.0.clamp(-PI / 2., PI / 2.));

			// Move camera laterally
			let camera_mat = camera.get_world();
			let basis_mat = Matrix3::from_cols(
				camera_mat.x.truncate(),
				camera_mat.y.truncate(),
				camera_mat.z.truncate(),
			);
			camera.position += basis_mat * heading * stats.delta_secs() * speed;
		}

		// Update title
		let title = format!("Crucible - TPS: {}", stats.tps().unwrap_or(0));
		for entity in wm.get_entities() {
			let viewport = wm.get_viewport(entity).unwrap();

			viewport.window().set_title(title.as_str());
		}

		input.end_tick();
	}

	fn draw(
		&mut self,
		_on_loop_ev: &mut VecDeque<RunLoopCommand>,
		engine: &Self::Engine,
		_event_loop: &EventLoopWindowTarget<()>,
		dep_guard: DepGuard,
		window: Entity,
		frame: &wgpu::SurfaceTexture,
	) {
		// Lock services
		let (vm, _) = dep_guard.get();
		get_many!(&**engine, gfx: &GfxContext, gfx_camera: &GfxCameraManager);
		lock_many_now!(
			engine.get_many() => _guard,
			uniform: &mut UniformManager,
			voxel: &mut VoxelRenderer,
			state: &mut GameState,
			world: &World,
		);

		// Construct uniforms
		match block_on(uniform.begin_frame()) {
			Ok(_) => {}
			Err(err) => {
				log::warn!("Failed to begin frame (uniform) {}", err);
				return;
			}
		}

		let viewport = vm.get_viewport(window).unwrap();
		let camera_group = gfx_camera.upload_view(
			gfx,
			uniform,
			state.camera.get_view_matrix(viewport.aspect()),
		);

		uniform.end_frame();

		// Create view
		let depth = state.depth.get_mut(world, window);
		{
			let current_depth_size = viewport.window().inner_size().to_vec();
			if depth.size != current_depth_size {
				depth.texture = create_depth_texture(gfx, current_depth_size);
				depth.size = current_depth_size;
			}
		}
		let depth_view = depth.texture.create_view(&Default::default());
		let frame_view = frame.texture.create_view(&Default::default());

		// Construct command buffer
		let mut cb = gfx
			.device
			.create_command_encoder(&wgpu::CommandEncoderDescriptor {
				label: Some("primary command encoder"),
			});

		let mut pass = cb.begin_render_pass(&wgpu::RenderPassDescriptor {
			label: None,
			color_attachments: &[wgpu::RenderPassColorAttachment {
				view: &frame_view,
				ops: wgpu::Operations {
					load: wgpu::LoadOp::Clear(wgpu::Color {
						r: 1.0,
						g: 1.0,
						b: 1.0,
						a: 1.0,
					}),
					store: true,
				},
				resolve_target: None,
			}],
			depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
				view: &depth_view,
				depth_ops: Some(wgpu::Operations {
					load: wgpu::LoadOp::Clear(1.),
					store: true,
				}),
				stencil_ops: None,
			}),
		});

		voxel.render(&world, &camera_group, &mut pass);
		drop(pass);
		gfx.queue.submit([cb.finish()]);
	}

	fn window_input(
		&mut self,
		on_loop_ev: &mut VecDeque<RunLoopCommand>,
		engine: &Self::Engine,
		_event_loop: &EventLoopWindowTarget<()>,
		dep_guard: DepGuard,
		window: Entity,
		event: &WindowEvent,
	) {
		// Lock services
		let (vm, _) = dep_guard.get();

		// Track inputs
		engine
			.get_lock::<InputTracker>()
			.lock_mut_now()
			.get()
			.handle_window_event(event);

		// Handle windowing events
		if let WindowEvent::CloseRequested = event {
			vm.unregister(vm.get_viewport(window).unwrap().id());

			if vm.get_entities().len() == 0 {
				on_loop_ev.fire(RunLoopCommand::Shutdown);
			}
		}
	}

	fn device_input(
		&mut self,
		_on_loop_ev: &mut VecDeque<RunLoopCommand>,
		engine: &Self::Engine,
		_event_loop: &EventLoopWindowTarget<()>,
		_dep_guard: DepGuard,
		device_id: DeviceId,
		event: &DeviceEvent,
	) {
		// Track inputs
		engine
			.get_lock::<InputTracker>()
			.lock_mut_now()
			.get()
			.handle_device_event(device_id, event);
	}

	fn goodbye(&mut self, _engine: &Self::Engine, _dep_guard: DepGuard) {}
}
