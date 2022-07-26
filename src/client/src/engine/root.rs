use std::cell::RefCell;

use anyhow::Context;
use geode::prelude::*;
use winit::{
	dpi::LogicalSize,
	event_loop::EventLoop,
	window::{Window, WindowBuilder},
};

use crate::{
	game::entry::GameSceneBundle,
	util::features::{FeatureDescriptor, FeatureList, FeatureScore},
};

use super::services::{
	gfx::{
		CompatQueryInfo, GfxContext, GfxFeatureDetector, GfxFeatureNeedsScreen,
		GfxFeaturePowerPreference,
	},
	input::InputTracker,
	resources::ResourceManager,
	scene::SceneManager,
	viewport::{
		DepthTextureKey, ScreenTexture, Viewport, ViewportManager, ViewportRenderEvent,
		DEFAULT_DEPTH_BUFFER_FORMAT,
	},
};

proxy_key! {
	pub struct MainLockKey of Owned<Lock>;
}

component_bundle! {
	pub struct EngineRootBundle(EngineRootBundleCtor) {
		gfx: GfxContext,
		viewport_mgr: RefCell<ViewportManager>,
		main_lock[MainLockKey::key()]: Owned<Lock>,
		scene_mgr: RefCell<SceneManager>,
		res_mgr: RefCell<ResourceManager>,
	}

	pub struct ViewportBundle(ViewportBundleCtor) {
		viewport: RefCell<Viewport>,
		input_tracker: RefCell<InputTracker>,
		render_handler: dyn EventHandlerOnce<ViewportRenderEvent>,
		depth_texture[DepthTextureKey::key()]: RefCell<ScreenTexture>,
	}
}

impl EngineRootBundle {
	pub fn new(
		s: Session,
		main_lock_guard: Owned<Lock>,
		event_loop: &EventLoop<()>,
	) -> anyhow::Result<Owned<Self>> {
		let (engine_guard, engine) = Entity::new(s).to_guard_ref_pair();
		let main_lock = main_lock_guard.weak_copy();

		// Create the main window for which we'll create our main surface
		let main_window = WindowBuilder::new()
			.with_title("Crucible")
			.with_inner_size(LogicalSize::new(1920u32, 1080u32))
			.with_visible(false)
			.build(event_loop)
			.context("failed to create main window")?;

		// Initialize a graphics context
		let (gfx, main_surface) = Self::init_gfx(&main_window)?;
		let (gfx_guard, gfx) = gfx.box_obj(s).to_guard_ref_pair();

		// Create `ViewportManager`
		let viewport_mgr_guard = ViewportManager::default().box_obj_rw(s, main_lock);
		let main_viewport = {
			let gfx = gfx.get(s);
			let (main_viewport_guard, main_viewport) =
				ViewportBundle::new(s, main_lock, gfx, engine).to_guard_ref_pair();

			viewport_mgr_guard.borrow_mut(s).register(
				s,
				main_lock,
				gfx,
				main_viewport_guard.map(ViewportBundle::raw),
				main_window,
				main_surface,
			);

			main_viewport
		};

		// Create `SceneManager`
		let (scene_mgr_guard, scene_mgr) = SceneManager::default()
			.box_obj_rw(s, main_lock)
			.to_guard_ref_pair();

		// Create resource manager
		let res_mgr_guard = ResourceManager::new(main_lock).box_obj_rw(s, main_lock);

		// Create root entity
		let (engine_guard, engine) = EngineRootBundle::add_onto_owned(
			s,
			engine_guard,
			EngineRootBundleCtor {
				gfx: gfx_guard.into(),
				viewport_mgr: viewport_mgr_guard.into(),
				main_lock: Obj::new(s, main_lock_guard).into(),
				scene_mgr: scene_mgr_guard.into(),
				res_mgr: res_mgr_guard.into(),
			},
		)
		.to_guard_ref_pair();

		// Setup initial scene
		scene_mgr
			.borrow_mut(s)
			.init_scene(GameSceneBundle::new(s, engine, main_viewport, main_lock).raw());

		Ok(engine_guard)
	}

	fn init_gfx(main_window: &Window) -> anyhow::Result<(GfxContext, wgpu::Surface)> {
		struct MyFeatureList;

		impl GfxFeatureDetector for MyFeatureList {
			type Table = ();

			fn query_compat(
				&mut self,
				info: &mut CompatQueryInfo,
			) -> (FeatureList, Option<Self::Table>) {
				let mut feature_list = FeatureList::default();

				feature_list.import_from(GfxFeatureNeedsScreen.query_compat(info).0);
				feature_list.import_from(
					GfxFeaturePowerPreference(wgpu::PowerPreference::HighPerformance)
						.query_compat(info)
						.0,
				);

				// Require wire-frame drawing
				// TODO: Make this an optional feature
				if feature_list.mandatory_feature(
					FeatureDescriptor {
						name: "Can Draw Wireframe",
						description: "",
					},
					if info
						.adapter_info
						.features
						.contains(wgpu::Features::POLYGON_MODE_LINE)
					{
						FeatureScore::BinaryPass
					} else {
						FeatureScore::BinaryFail {
							reason: "`POLYGON_MODE_LINE` feature not supported by the adapter"
								.to_string(),
						}
					},
				) {
					info.descriptor
						.features
						.insert(wgpu::Features::POLYGON_MODE_LINE);
				}

				feature_list.wrap_user_table(())
			}
		}

		let (gfx, _, main_surface) =
			futures::executor::block_on(GfxContext::init(&main_window, &mut MyFeatureList))
				.context("failed to create graphics context")?;

		Ok((gfx, main_surface))
	}
}

impl ViewportBundle {
	pub fn new(s: Session, main_lock: Lock, gfx: &GfxContext, engine: Entity) -> Owned<Self> {
		// Construct main viewport
		let input_tracker = InputTracker::default().box_obj_rw(s, main_lock);
		let render_handler = Obj::new(
			s,
			move |s: Session, _me: Entity, event: ViewportRenderEvent| {
				let p_scene_mgr = engine.borrow::<SceneManager>(s);
				let current_scene = p_scene_mgr.current_scene();

				current_scene
					.get::<dyn EventHandlerOnce<ViewportRenderEvent>>(s)
					.fire(s, current_scene, event);
			},
		)
		.unsize::<dyn EventHandlerOnce<ViewportRenderEvent>>();

		let depth_texture = ScreenTexture::new(
			gfx,
			Some("depth buffer"),
			DEFAULT_DEPTH_BUFFER_FORMAT,
			wgpu::TextureUsages::RENDER_ATTACHMENT,
		)
		.box_obj_rw(s, main_lock);

		Self::spawn(
			s,
			ViewportBundleCtor {
				viewport: None, // (to be initialized by the viewport manager)
				input_tracker: input_tracker.into(),
				render_handler: render_handler.into(),
				depth_texture: depth_texture.into(),
			},
		)
	}
}
