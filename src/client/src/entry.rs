//! Entry point for Crucible. This is placed in a separate module from `main.rs` to avoid polluting
//! the main namespace.

use crate::engine::context::GfxContext;
use crate::engine::input::InputTracker;
use crate::engine::run_loop::{
	start_run_loop, DepGuard, RunLoopCommand, RunLoopHandler, RunLoopTiming,
};
use crate::engine::util::camera::{update_camera_free_cam, GfxCameraManager, PerspectiveCamera};
use crate::engine::util::uniform::UniformManager;
use crate::engine::viewport::{DepthTextureManager, ViewportManager};
use crate::voxel::render::VoxelRenderer;
use anyhow::Context;
use cgmath::Vector3;
use crucible_core::foundation::prelude::*;
use crucible_core::util::error::AnyResult;
use crucible_core::util::format::FormatMs;
use crucible_core::util::meta_enum::EnumMeta;
use crucible_shared::voxel::coord::{Axis3, BlockPos, ChunkPos};
use crucible_shared::voxel::data::VoxelWorld;
use futures::executor::block_on;
use rand::random;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;
use winit::dpi::LogicalSize;
use winit::event::{DeviceEvent, DeviceId, MouseButton, VirtualKeyCode, WindowEvent};
use winit::event_loop::{EventLoop, EventLoopWindowTarget};
use winit::window::WindowBuilder;

provider_struct! {
	pub struct Engine {
		// Foundational services
		rw_mgr: RwLockManager,
		world: RwLock<World>,

		// Core engine services
		gfx: GfxContext,
		vm: RwLock<ViewportManager>,
		vm_depth: RwLock<DepthTextureManager>,
		input: RwLock<InputTracker>,
		uniform: RwLock<UniformManager>,
		camera_mgr: RwLock<GfxCameraManager>,
		timings: RwLock<RunLoopTiming>,

		// Game services
		voxel_data: RwLock<VoxelWorld>,
		voxel_render: RwLock<VoxelRenderer>,
		game_state: RwLock<GameState>,
	}
}

impl Engine {
	pub fn start() -> AnyResult<!> {
		// Initialize foundational services
		env_logger::init();

		let mut world = World::default();

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
		let timings = RunLoopTiming::start(u32::MAX);
		let input = InputTracker::new();
		let uniform = UniformManager::new(
			&gfx,
			Some("uniform manager"),
			wgpu::BufferUsages::UNIFORM,
			1024,
		);

		let camera_mgr = GfxCameraManager::new(&gfx);

		// Setup voxels
		log::info!("Building world data...");
		let mut voxel_data = VoxelWorld::new();
		let mut voxel_render = VoxelRenderer::new(&gfx, &camera_mgr);

		let multi_cube_dim = Vector3::new(8, 8, 8);

		for x in 0..=multi_cube_dim.x {
			for y in 0..=multi_cube_dim.x {
				for z in 0..=multi_cube_dim.x {
					let ent_chunk = world.spawn();

					// Setup chunk data
					let chunk_pos = ChunkPos::new(x, y, z);
					voxel_data.add(&world, chunk_pos, ent_chunk);

					let mut data = voxel_data.get_chunk_mut(&world, ent_chunk).unwrap();
					data.set_block(BlockPos::new(0, 0, 0), 1);

					let mut is_complete = true;

					for (axis, _) in Axis3::values_iter() {
						if chunk_pos.raw[axis.vec_idx] == multi_cube_dim[axis.vec_idx] {
							is_complete = false;
							continue;
						}

						for d in 1..16 {
							data.set_block(BlockPos::new(0, 0, 0) + (axis.unit() * d), 1);
						}
					}

					if is_complete && random::<bool>() {
						for b_x in 1..16 {
							for b_y in 1..16 {
								for b_z in 1..16 {
									if random::<bool>() {
										data.set_block(BlockPos::new(b_x, b_y, b_z), 1);
									}
								}
							}
						}
					}

					// Mesh chunk
					voxel_render.mark_dirty(&world, ent_chunk);
				}
			}
		}
		log::info!("Done building world data.");

		// Create game state
		let mut vm_depth = DepthTextureManager::new();
		for e_win in vm.get_entities() {
			let viewport = vm.get_viewport(e_win).unwrap();
			viewport.window().set_visible(true);
			vm_depth.register(&world, &gfx, viewport);
		}

		let game_state = GameState {
			camera: PerspectiveCamera {
				position: Vector3::new(0., 0., 10.),
				..Default::default()
			},
			is_active: false,
			main_window,
		};

		// Wrap services
		let rw_mgr = RwLockManager::default();
		let world = RwLock::new(rw_mgr.clone(), world);
		let vm = RwLock::new(rw_mgr.clone(), vm);
		let vm_depth = RwLock::new(rw_mgr.clone(), vm_depth);
		let input = RwLock::new(rw_mgr.clone(), input);
		let uniform = RwLock::new(rw_mgr.clone(), uniform);
		let camera_mgr = RwLock::new(rw_mgr.clone(), camera_mgr);
		let timings = RwLock::new(rw_mgr.clone(), timings);
		let voxel_data = RwLock::new(rw_mgr.clone(), voxel_data);
		let voxel_render = RwLock::new(rw_mgr.clone(), voxel_render);
		let game_state = RwLock::new(rw_mgr.clone(), game_state);

		// Start run loop
		log::info!("Starting run loop!");
		let engine = Arc::new(Self {
			rw_mgr,
			world,
			gfx,
			vm,
			vm_depth,
			input,
			uniform,
			camera_mgr,
			timings,
			voxel_data,
			voxel_render,
			game_state,
		});
		start_run_loop(event_loop, engine);
	}
}

impl RunLoopHandler for Arc<Engine> {
	fn tick(
		&self,
		on_loop_ev: &mut VecDeque<RunLoopCommand>,
		_event_loop: &EventLoopWindowTarget<()>,
		dep_guard: DepGuard,
	) {
		// Lock services
		let (wm, timings) = dep_guard.get();
		let gfx = &self.gfx;
		lock_many_now!(
			self.get_many() => _guard,
			world: &World,
			input: &mut InputTracker,
			state: &mut GameState,
			voxel_data: &VoxelWorld,
			voxel_render: &mut VoxelRenderer,
		);

		// Update chunks
		block_on(voxel_render.update_dirty(world, voxel_data, gfx, Duration::from_millis(10)));

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
				let window = wm
					.get_viewport(state.main_window)
					.unwrap()
					.component()
					.window();
				let _ = window.set_cursor_grab(state.is_active);
				window.set_cursor_visible(!state.is_active);
			}
		}

		if state.is_active {
			update_camera_free_cam(&mut state.camera, &input, &timings);
		}

		// Update title
		let title = format!(
			"Crucible - TPS: {} - MSPT: {}, MSPF: {}",
			timings.tps().unwrap_or(0),
			FormatMs(timings.mspt().unwrap_or(Duration::ZERO)),
			FormatMs(timings.mspf().unwrap_or(Duration::ZERO))
		);
		for entity in wm.get_entities() {
			let viewport = wm.get_viewport(entity).unwrap();

			viewport.window().set_title(title.as_str());
		}

		input.end_tick();
	}

	fn draw(
		&self,
		_on_loop_ev: &mut VecDeque<RunLoopCommand>,
		_event_loop: &EventLoopWindowTarget<()>,
		dep_guard: DepGuard,
		window: Entity,
		frame: &wgpu::SurfaceTexture,
	) {
		// Lock services
		let (vm, _) = dep_guard.get();
		let gfx = &self.gfx;
		lock_many_now!(
			self.get_many() => _guard,
			uniform: &mut UniformManager,
			voxel: &mut VoxelRenderer,
			state: &mut GameState,
			vm_depth: &mut DepthTextureManager,
			camera_mgr: &GfxCameraManager,
			world: &World,
		);

		// Construct uniforms
		let viewport = vm.get_viewport(window).unwrap();
		let camera_group = camera_mgr.upload_view(
			gfx,
			uniform,
			state.camera.get_view_matrix(viewport.aspect()),
		);

		uniform.flush(gfx);

		// Create view
		let depth_view = vm_depth.present(world, gfx, viewport);
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
		&self,
		on_loop_ev: &mut VecDeque<RunLoopCommand>,
		_event_loop: &EventLoopWindowTarget<()>,
		dep_guard: DepGuard,
		window: Entity,
		event: &WindowEvent,
	) {
		// Lock services
		let (vm, _) = dep_guard.get();

		// Track inputs
		self.get_lock::<InputTracker>()
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
		&self,
		_on_loop_ev: &mut VecDeque<RunLoopCommand>,
		_event_loop: &EventLoopWindowTarget<()>,
		_dep_guard: DepGuard,
		device_id: DeviceId,
		event: &DeviceEvent,
	) {
		// Track inputs
		self.get_lock::<InputTracker>()
			.lock_mut_now()
			.get()
			.handle_device_event(device_id, event);
	}

	fn goodbye(&self, _dep_guard: DepGuard) {}
}

struct GameState {
	camera: PerspectiveCamera,
	is_active: bool,
	main_window: Entity,
}
