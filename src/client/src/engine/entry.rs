use anyhow::Context;
use crucible_core::ecs::{
	core::{Archetype, Entity, Storage},
	userdata::Userdata,
};
use wgpu::SurfaceConfiguration;
use winit::{event_loop::EventLoop, window::WindowBuilder};

use super::{
	gfx::{GfxContext, GfxFeatureNeedsScreen},
	input::InputManager,
	resources::ResourceManager,
	scene::SceneManager,
	viewport::{Viewport, ViewportManager},
};

struct EngineRoot {
	// Services
	gfx: GfxContext,
	res_mgr: ResourceManager,
	input_mgr: InputManager,
	viewport_mgr: ViewportManager,
	scene_mgr: SceneManager,

	// Archetypes
	viewport_arch: Archetype,
	scene_arch: Archetype,

	// Storages
	viewports: Storage<Viewport>,
	userdata: Storage<Userdata>,
	update_handlers: Storage<fn(Entity, &mut Storage<Userdata>)>,
	render_handlers: Storage<
		fn(
			Entity,
			(
				&mut Storage<Userdata>,
				&GfxContext,
				&mut wgpu::SurfaceTexture,
			),
		),
	>,
}

impl EngineRoot {
	async fn new() -> anyhow::Result<(EventLoop<()>, Self)> {
		// Create main window
		let event_loop = EventLoop::new();
		let main_window = WindowBuilder::new()
			.with_title("Crucible")
			.with_visible(false)
			.build(&event_loop)
			.context("failed to create main window")?;

		// Create graphics subsystem
		let (gfx, _compat, main_surface) =
			GfxContext::init(&main_window, &mut GfxFeatureNeedsScreen)
				.await
				.context("failed to create graphics context")?;

		// Create main viewport
		let mut viewport_mgr = ViewportManager::default();
		let mut viewport_arch = Archetype::new();
		let mut viewports = Storage::new();

		let main_viewport = viewport_arch.spawn();
		viewports.add(
			main_viewport,
			Viewport::new(
				(&gfx,),
				main_window,
				Some(main_surface),
				SurfaceConfiguration {
					alpha_mode: wgpu::CompositeAlphaMode::Auto,
					format: wgpu::TextureFormat::Bgra8UnormSrgb,
					present_mode: wgpu::PresentMode::Fifo,
					usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
					width: 0,
					height: 0,
				},
			),
		);

		viewport_mgr.register((&viewports,), main_viewport);

		// Create input tracker
		let input_mgr = InputManager::default();

		// Create scene manager
		let scene_mgr = SceneManager::default();
		let scene_arch = Archetype::new();
		let userdata = Storage::new();
		let update_handlers = Storage::new();
		let render_handlers = Storage::new();

		// Create resource manager
		let res_mgr = ResourceManager::default();

		Ok((
			event_loop,
			Self {
				// Services
				gfx,
				res_mgr,
				input_mgr,
				viewport_mgr,
				scene_mgr,

				// Archetypes
				viewport_arch,
				scene_arch,

				// Storages
				viewports,
				userdata,
				update_handlers,
				render_handlers,
			},
		))
	}
}

pub async fn main_inner() -> anyhow::Result<()> {
	let (event_loop, mut root) = EngineRoot::new().await?;

	// Make all viewports visible
	{
		let viewports = &mut root.viewports;
		for &viewport in root.viewport_mgr.window_map().values() {
			viewports[viewport].window().set_visible(true);
		}
	}

	// Setup initial scene
	{
		let scene_mgr = &mut root.scene_mgr;

		let userdata = &mut root.userdata;
		let scene_arch = &mut root.scene_arch;
		let update_handlers = &mut root.update_handlers;
		let render_handlers = &mut root.render_handlers;

		let scene = scene_arch.spawn();
		userdata.add(scene, Box::new(0f64));

		update_handlers.add(scene, |me, userdata| {
			let me_data = userdata.get_downcast_mut::<f64>(me);
			*me_data += 0.01;
		});

		render_handlers.add(scene, |me, (userdata, gfx, frame)| {
			let me_data = userdata.get_downcast::<f64>(me);

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

			let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
				label: Some("main render pass"),
				color_attachments: &[Some(wgpu::RenderPassColorAttachment {
					view: &view,
					ops: wgpu::Operations {
						load: wgpu::LoadOp::Clear(wgpu::Color {
							r: 0.5 + 0.5 * me_data.cos(),
							g: 0.1,
							b: 0.1,
							a: 1.0,
						}),
						store: true,
					},
					resolve_target: None,
				})],
				depth_stencil_attachment: None,
			});

			drop(pass);

			gfx.queue.submit([encoder.finish()]);
		});

		scene_mgr.set_initial(scene);
	}

	// Run event loop
	event_loop.run(move |event, _proxy, flow| {
		use winit::event::{Event::*, WindowEvent::*};

		flow.set_poll();

		match event {
			// First window events and device events are dispatched.
			WindowEvent { window_id, event } => {
				let Some(_viewport) = root.viewport_mgr.get_viewport(window_id) else {
					return;
				};

				// Process window event
				root.input_mgr.handle_window_event(&event);

				if let CloseRequested = event {
					flow.set_exit();
				}
			}
			DeviceEvent { device_id, event } => {
				root.input_mgr.handle_device_event(device_id, &event);
			}
			MainEventsCleared => {
				// Process update
				let curr_scene = root.scene_mgr.current();
				root.update_handlers[curr_scene](curr_scene, &mut root.userdata);

				// Request redraws
				for (&_window, &viewport) in root.viewport_mgr.window_map() {
					root.viewports[viewport].window().request_redraw();
				}
			}

			// Then, redraws are processed.
			RedrawRequested(window_id) => {
				let Some(viewport) = root.viewport_mgr.get_viewport(window_id) else {
					return;
				};

				// Acquire frame
				let viewport_data = &mut root.viewports[viewport];
				let Ok(surface) = viewport_data.present((&root.gfx,)) else {
					log::error!("Failed to render to {viewport:?}");
					return;
				};

				let Some(mut frame) = surface else {
					return;
				};

				// Process render
				let curr_scene = root.scene_mgr.current();
				root.render_handlers[curr_scene](
					curr_scene,
					(&mut root.userdata, &root.gfx, &mut frame),
				);

				frame.present();
			}
			RedrawEventsCleared => {}

			// This runs at program termination.
			LoopDestroyed => {
				log::info!("Goodbye!");
			}
			_ => {}
		}
	});
}
