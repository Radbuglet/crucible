use anyhow::Context;
use crucible_common::game::material::MaterialStateBase;
use crucible_util::debug::userdata::BoxedUserdata;
use geode::prelude::*;
use wgpu::SurfaceConfiguration;
use winit::{
	dpi::LogicalSize,
	event::{DeviceEvent, DeviceId, WindowEvent},
	event_loop::EventLoopBuilder,
	window::{WindowBuilder, WindowId},
};

use crate::{
	engine::scene::{SceneRenderEvent, SceneUpdateEvent},
	game::{
		entry::PlaySceneState,
		voxel::material::{
			BasicMaterialDescriptorBundle, InvisibleBlockDescriptorBundle, MaterialStateVisualBlock,
		},
	},
};

use super::{
	assets::AssetManager,
	gfx::texture::FullScreenTexture,
	io::{
		gfx::{GfxContext, GfxFeatureNeedsScreen},
		input::InputManager,
		main_loop::{MainLoop, MainLoopHandler, WinitEventLoop, WinitEventProxy},
		viewport::{Viewport, ViewportBundle, ViewportManager},
	},
	scene::{SceneBundle, SceneManager, SceneRenderHandler, SceneUpdateHandler},
};

// === EngineRoot === //

#[derive(Debug)]
struct EngineRoot {
	universe: Universe,
	gfx: GfxContext,
	asset_mgr: AssetManager,
	viewport_mgr: ViewportManager,
	scene_mgr: SceneManager,
}

impl EngineRoot {
	fn new() -> anyhow::Result<(WinitEventLoop, Self)> {
		// Create universe and acquire context
		let universe = Universe::new();
		let mut guard;
		let mut cx = unpack!(&universe => guard & (
			&Universe,
			@arch ViewportBundle,
			@arch SceneBundle,
			@mut Storage<Viewport>,
			@mut Storage<InputManager>,
			@mut Storage<FullScreenTexture>,
			@mut Storage<BoxedUserdata>,
			@mut Storage<SceneUpdateHandler>,
			@mut Storage<SceneRenderHandler>,
		));

		// Create main window
		let event_loop = EventLoopBuilder::with_user_event().build();
		let main_window = WindowBuilder::new()
			.with_title("Crucible")
			.with_inner_size(LogicalSize::new(1920, 1080))
			.with_visible(false)
			.build(&event_loop)
			.context("failed to create main window")?;

		// Create graphics subsystem
		let (gfx, _compat, main_surface) =
			futures::executor::block_on(GfxContext::init(&main_window, &mut GfxFeatureNeedsScreen))
				.context("failed to create graphics context")?;

		// Create main viewport
		let mut viewport_mgr = ViewportManager::default();
		let main_viewport = {
			decompose!(cx => cx & { viewport_arch: &mut Archetype<ViewportBundle> });
			viewport_arch.spawn_with(
				decompose!(cx),
				"my viewport",
				ViewportBundle::new(Viewport::new(
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
				)),
			)
		};

		decompose!(cx => { viewports: &Storage<Viewport> });
		viewport_mgr.register((viewports,), main_viewport);

		// Create other services
		let scene_mgr = SceneManager::default();
		let asset_mgr = AssetManager::default();

		// Construct `EngineRoot`
		drop(guard);

		Ok((
			event_loop,
			Self {
				universe,
				gfx,
				asset_mgr,
				viewport_mgr,
				scene_mgr,
			},
		))
	}
}

impl MainLoopHandler for EngineRoot {
	fn on_update(&mut self, (main_loop, proxy): (&mut MainLoop, &WinitEventProxy)) {
		// Update current scene
		unpack!(&self.universe => {
			update_handlers: @ref Storage<SceneUpdateHandler>,
		});

		let scene = self.scene_mgr.current();
		update_handlers[scene](
			&Provider::new_with(
				&self.universe,
				(&self.gfx, &mut self.asset_mgr, main_loop, proxy),
			),
			scene,
			SceneUpdateEvent {},
		);

		// Request redraws
		unpack!(&self.universe => {
			viewports: @ref Storage<Viewport>,
			input_managers: @mut Storage<InputManager>,
		});

		for (&_window, &viewport) in self.viewport_mgr.window_map() {
			viewports[viewport].window().request_redraw();
		}

		// Clear input queue
		for (_, &viewport) in self.viewport_mgr.window_map() {
			input_managers[viewport].end_tick();
		}
	}

	fn on_render(
		&mut self,
		(main_loop, proxy): (&mut MainLoop, &WinitEventProxy),
		window_id: WindowId,
	) {
		// Acquire context
		unpack!(&self.universe => {
			viewports: @mut Storage<Viewport>,
			render_handlers: @ref Storage<SceneRenderHandler>,
		});

		// Acquire viewport
		let Some(viewport) = self.viewport_mgr.get_viewport(window_id) else {
			return;
		};

		// Acquire frame
		let viewport_data = &mut viewports[viewport];
		let Ok(surface) = viewport_data.present((&self.gfx,)) else {
			log::error!("Failed to render to {viewport:?}");
			return;
		};

		let Some(mut frame) = surface else {
			return;
		};

		// Process render
		let curr_scene = self.scene_mgr.current();
		let cx = Provider::new_with(
			&self.universe,
			(
				&self.gfx,
				&mut self.asset_mgr,
				&mut *viewports,
				main_loop,
				proxy,
			),
		);
		render_handlers[curr_scene](&cx, curr_scene, SceneRenderEvent { frame: &mut frame });

		frame.present();
	}

	fn on_window_input(
		&mut self,
		(main_loop, _proxy): (&mut MainLoop, &WinitEventProxy),
		window_id: WindowId,
		event: WindowEvent,
	) {
		unpack!(&self.universe => {
			input_managers: @mut Storage<InputManager>,
		});

		let Some(viewport) = self.viewport_mgr.get_viewport(window_id) else {
			log::warn!("Unknown viewport with window ID {window_id:?}");
			return;
		};

		input_managers[viewport].handle_window_event(&event);

		if let WindowEvent::CloseRequested = &event {
			main_loop.exit();
		}
	}

	fn on_device_input(
		&mut self,
		(_main_loop, _proxy): (&mut MainLoop, &WinitEventProxy),
		device_id: DeviceId,
		event: DeviceEvent,
	) {
		unpack!(&self.universe => {
			input_managers: @mut Storage<InputManager>,
		});

		for &viewport in self.viewport_mgr.window_map().values() {
			input_managers[viewport].handle_device_event(device_id, &event);
		}
	}
}

// === Main === //

pub fn main_inner() -> anyhow::Result<()> {
	// Construct engine root
	let (event_loop, mut root) = EngineRoot::new()?;

	// Setup initial scene
	let scene_mgr = &mut root.scene_mgr;
	let &main_viewport = root.viewport_mgr.window_map().values().next().unwrap();

	let scene = {
		// Acquire context
		unpack!(cx_full & cx = &root.universe => {
			scene_bundle: @arch SceneBundle,
			...:
				&Universe,
				@mut Storage<BoxedUserdata>,
				@mut Storage<SceneUpdateHandler>,
				@mut Storage<SceneRenderHandler>,
				@arch InvisibleBlockDescriptorBundle,
				@arch BasicMaterialDescriptorBundle,
				@mut Storage<MaterialStateBase>,
				@mut Storage<MaterialStateVisualBlock>,
		});
		let mut cx = (cx, (&root.gfx, &mut root.asset_mgr));

		// Create scene
		let mut scene = PlaySceneState::new(decompose!(cx), main_viewport);

		// Load assets
		scene.create_default_materials(decompose!(cx));
		scene.upload_atlases(decompose!(cx));

		// Construct entity
		scene_bundle.spawn_with(decompose!(cx), "play scene", scene.make_bundle())
	};

	scene_mgr.set_initial(scene);

	// Make all viewports visible
	{
		unpack!(&root.universe => {
			viewports: @ref Storage<Viewport>,
		});

		for &viewport in root.viewport_mgr.window_map().values() {
			viewports[viewport].window().set_visible(true);
		}
	}

	// Run event loop
	MainLoop::start(event_loop, root);
}
