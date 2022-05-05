use crate::engine::services::gfx::{
	CompatQueryInfo, GfxContext, GfxFeatureDetector, GfxFeatureNeedsScreen,
	GfxFeaturePowerPreference,
};
use crate::util::features::FeatureList;
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
	event_loop.run(|_event, _proxy, _flow| {});
}

async fn make_engine_root(event_loop: &EventLoop<()>) -> anyhow::Result<Obj> {
	let mut root = Obj::new();

	// Create core services
	root.add_rw(World::new());

	// Create graphics subsystem
	{
		let main_window = WindowBuilder::new()
			.with_title("Crucible")
			.with_inner_size(LogicalSize::new(1920, 1080))
			.with_visible(false)
			.build(event_loop)
			.context("failed to create main window")?;

		let gfx = GfxContext::init(&main_window, &mut MyFeatureListValidator)
			.await
			.context("failed to create graphics context")?;

		root.add(gfx);
	}

	Ok(root)
}

struct MyFeatureListValidator;

impl GfxFeatureDetector for MyFeatureListValidator {
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
