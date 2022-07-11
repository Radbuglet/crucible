use std::cell::RefCell;

use anyhow::Context;
use geode::prelude::*;
use winit::{dpi::LogicalSize, event_loop::EventLoop, window::WindowBuilder};

use crate::{game::entry::make_game_entry, util::features::FeatureList};

use super::services::{
	gfx::{
		CompatQueryInfo, GfxContext, GfxFeatureDetector, GfxFeatureNeedsScreen,
		GfxFeaturePowerPreference,
	},
	input::InputTracker,
	scene::SceneManager,
	viewport::{ViewportManager, ViewportRenderHandler},
};

proxy_key! {
	pub struct MainLockKey of Owned<Lock>;
}

component_bundle! {
	pub struct EngineRootBundle {
		gfx: GfxContext,
		viewport_mgr: RefCell<ViewportManager>,
		main_lock[MainLockKey::key()]: Owned<Lock>,
		scene_mgr: RefCell<SceneManager>,
	}
}

impl EngineRootBundle {
	pub fn new(
		s: Session,
		main_lock_guard: Owned<Lock>,
		event_loop: &EventLoop<()>,
	) -> anyhow::Result<Owned<Self>> {
		let engine_root_guard = Entity::new(s);
		let engine_root = *engine_root_guard;
		let main_lock = *main_lock_guard;

		// Create the main window for which we'll create our main surface.
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

		let gfx_guard = gfx.box_obj(s);
		let gfx = *gfx_guard;

		engine_root_guard.add(s, gfx_guard);

		// Create `ViewportManager`
		let viewport_mgr = ViewportManager::default().box_obj_rw(s, main_lock);
		{
			// Acquire services
			let mut p_viewport_mgr = viewport_mgr.borrow_mut(s);
			let p_gfx = gfx.get(s);

			// Construct main viewport
			let input_mgr = InputTracker::default().box_obj_rw(s, main_lock);
			let render_handler = Obj::new(s, move |frame, s: Session, _me, viewport, engine| {
				let p_scene_mgr = engine_root.borrow::<SceneManager>(s);
				let current_scene = p_scene_mgr.current_scene();

				current_scene.get::<dyn ViewportRenderHandler>(s).on_render(
					frame,
					s,
					current_scene,
					viewport,
					engine,
				);
			})
			.to_unsized::<dyn ViewportRenderHandler>();

			let main_viewport = Entity::new_with(s, (render_handler, input_mgr));

			// Register main viewport
			p_viewport_mgr.register(
				s,
				main_lock,
				p_gfx,
				main_viewport,
				main_window,
				main_surface,
			);
		}

		// Create `SceneManager`
		let scene_mgr = SceneManager::default().box_obj_rw(s, main_lock);
		scene_mgr
			.borrow_mut(s)
			.init_scene(make_game_entry(s, *engine_root_guard, main_lock));

		// Create root entity
		engine_root_guard.add(
			s,
			(
				viewport_mgr,
				scene_mgr,
				ExposeUsing(main_lock_guard.box_obj(s), MainLockKey::key()),
			),
		);

		Ok(engine_root_guard.map_owned(Self::unchecked_cast))
	}
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
