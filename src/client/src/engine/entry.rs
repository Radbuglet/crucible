use anyhow::Context;
use geode::prelude::*;
use winit::{dpi::LogicalSize, event_loop::EventLoop, window::WindowBuilder};

use crate::{game::entry::make_game_entry, util::features::FeatureList};

use super::{
	gfx::{
		CompatQueryInfo, GfxContext, GfxFeatureDetector, GfxFeatureNeedsScreen,
		GfxFeaturePowerPreference,
	},
	input::InputTracker,
	scene::SceneManager,
	viewport::ViewportManager,
};

pub fn main_inner() -> anyhow::Result<()> {
	// Create main thread lock.
	let (mut main_lock_token, main_lock) = LockToken::new("main thread");

	// Create our main session.
	let session = Session::new([&mut main_lock_token]);
	let s = &session;

	// Create the main window for which we'll create our main surface.
	let event_loop = EventLoop::new();
	let main_window = WindowBuilder::new()
		.with_title("Crucible")
		.with_inner_size(LogicalSize::new(1920u32, 1080u32))
		.with_visible(false)
		.build(&event_loop)
		.context("failed to create main window")?;

	// Initialize a graphics context.
	let (gfx, _table, main_surface) =
		futures::executor::block_on(GfxContext::init(&main_window, &mut MyFeatureList))
			.context("failed to create graphics context")?;

	let gfx = gfx.box_obj(s);

	// Create `ViewportManager`
	let viewport_mgr = ViewportManager::default().box_obj_rw(s, main_lock);
	{
		let mut viewport_mgr_p = viewport_mgr.borrow_mut(s);
		let gfx_p = gfx.get(s);

		let main_viewport = Entity::new(s);

		viewport_mgr_p.register(
			s,
			main_lock,
			gfx_p,
			main_viewport,
			main_window,
			main_surface,
		);
	}

	// Create `InputTracker`
	let input_mgr = InputTracker::default().box_obj_rw(s, main_lock);

	// Create `SceneManager`
	let scene_mgr = SceneManager::default().box_obj_rw(s, main_lock);
	scene_mgr
		.borrow_mut(s)
		.init_scene(make_game_entry(s, main_lock));

	// Create root entity
	let root = Entity::new(s);
	root.add(s, (gfx, viewport_mgr, input_mgr, scene_mgr));

	// Start engine
	{
		let viewport_mgr_p = root.borrow::<ViewportManager>(s);

		for (_, _viewport, window) in viewport_mgr_p.mounted_viewports(s) {
			window.set_visible(true);
		}
	}

	event_loop.run(move |_event, _proxy, _flow| {});
}

struct MyFeatureList;

impl GfxFeatureDetector for MyFeatureList {
	type Table = ();

	fn query_compat(&mut self, info: &mut CompatQueryInfo) -> (FeatureList, Option<Self::Table>) {
		let mut feature_list = FeatureList::default();

		feature_list.import_from(GfxFeatureNeedsScreen.query_compat(info).0);
		feature_list.import_from(
			GfxFeaturePowerPreference(wgpu::PowerPreference::HighPerformance)
				.query_compat(info)
				.0,
		);

		feature_list.wrap_user_table(())
	}
}
