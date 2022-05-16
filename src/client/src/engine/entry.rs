use crate::engine::gfx::{
	CompatQueryInfo, GfxContext, GfxFeatureDetector, GfxFeatureNeedsScreen,
	GfxFeaturePowerPreference,
};
use crate::engine::scene::SceneManager;
use crate::engine::viewport::{Viewport, ViewportManager};
use crate::game::entry::make_game_scene;
use crate::util::features::FeatureList;
use crate::util::winit::WinitEventHandler;
use crate::util::winit::{WinitEventBundle, WinitUserdata};
use anyhow::Context;
use futures::executor::block_on;
use geode::prelude::*;
use winit::dpi::LogicalSize;
use winit::event_loop::EventLoop;
use winit::window::WindowBuilder;

pub fn main_inner() -> anyhow::Result<()> {
	// Initialize logger
	env_logger::init();
	log::info!("Hello!");

	// Initialize engine
	let event_loop = EventLoop::new();
	let root = block_on(make_engine_root(&event_loop)).context("failed to initialize engine")?;

	// Run engine
	log::info!("Main loop starting.");
	root.inject(|vm: ARef<ViewportManager>| {
		for (_, viewport_obj) in vm.viewports() {
			viewport_obj.borrow::<Viewport>().window().set_visible(true);
		}
	});

	event_loop.run(move |event, proxy, flow| {
		let mut bundle = WinitEventBundle { event, proxy, flow };
		let sm = root.borrow::<SceneManager>();
		sm.current_scene()
			.get::<dyn WinitEventHandler>()
			.on_winit_event(&mut ObjCx::with_root(&root), &mut bundle);
		drop(sm);
		let mut sm = root.borrow_mut::<SceneManager>();
		sm.swap_scenes();
	});
}

async fn make_engine_root(event_loop: &EventLoop<WinitUserdata>) -> anyhow::Result<Obj> {
	let mut root = Obj::labeled("engine root");

	// Create core services
	root.add_rw(World::new());

	// Create graphics subsystem
	{
		// Create context
		let main_window = WindowBuilder::new()
			.with_title("Crucible")
			.with_inner_size(LogicalSize::new(1920u32, 1080u32))
			.with_visible(false)
			.build(event_loop)
			.context("failed to create main window")?;

		let (gfx, _gfx_features, main_swapchain) =
			GfxContext::init(&main_window, &mut CustomFeatureListValidator)
				.await
				.context("failed to create graphics context")?;

		root.add(gfx);

		// Setup viewport manager
		let gfx = root.get::<GfxContext>();
		let mut vm = ViewportManager::default();
		vm.register(gfx, Obj::new(), main_window, main_swapchain);
		root.add_rw(vm);
	};

	// Register game subsystems
	{
		let mut sm = SceneManager::default();
		sm.init_scene(make_game_scene());
		root.add_rw(sm);
	}

	Ok(root)
}

struct CustomFeatureListValidator;

impl GfxFeatureDetector for CustomFeatureListValidator {
	type Table = ();

	fn query_compat(&mut self, info: &mut CompatQueryInfo) -> (FeatureList, Option<Self::Table>) {
		let mut features = FeatureList::default();
		features.import_from(GfxFeatureNeedsScreen.query_compat(info).0);
		features.import_from(
			GfxFeaturePowerPreference(wgpu::PowerPreference::HighPerformance)
				.query_compat(info)
				.0,
		);
		features.wrap_user_table(())
	}
}
