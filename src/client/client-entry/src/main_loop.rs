use std::{marker::PhantomData, sync::Arc, time::Instant};

use anyhow::Context;
use bevy_app::{App, Update};
use bevy_autoken::{
    despawn_entity, spawn_entity, world_mut, RandomAccess, RandomAppExt, RandomEntityExt,
    RandomWorldExt,
};
use bevy_ecs::{
    entity::Entity,
    schedule::IntoSystemConfigs,
    system::{Res, Resource},
};
use crucible_assets::AssetManager;
use main_loop::{
    feat_requires_screen, run_app_with_init, sys_unregister_dead_viewports, GfxContext,
    InputManager, LimitedRate, Viewport, ViewportManager,
};
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, DeviceId, StartCause, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::NamedKey,
    window::{Window, WindowId},
};

use crate::{
    dummy_game::{sys_process_camera_controller, PlayerCameraController},
    render::{
        helpers::{CameraManager, VirtualCamera},
        ViewportRenderer, ViewportRendererCx,
    },
};

pub fn main_inner() -> anyhow::Result<()> {
    // Build event loop and start app!
    let event_loop = EventLoop::new().context("failed to create event loop")?;

    run_app_with_init(event_loop, |event_loop| {
        // Create app
        let mut app = App::new();

        app.add_random_component::<AssetManager>();
        app.add_random_component::<CameraManager>();
        app.add_random_component::<GfxContext>();
        app.add_random_component::<InputManager>();
        app.add_random_component::<PlayerCameraController>();
        app.add_random_component::<Viewport>();
        app.add_random_component::<ViewportManager>();
        app.add_random_component::<ViewportRenderer>();
        app.add_random_component::<VirtualCamera>();

        #[rustfmt::skip]
        app.add_systems(
            Update,
            (
                sys_process_camera_controller,
                sys_handle_esc_to_exit,
                sys_unregister_dead_viewports,
                sys_reset_input_tracker,
            )
            .chain(),
        );

        // Initialize engine root
        let engine_root = app.use_random(|cx| init_engine_root(cx, event_loop))?;
        app.insert_resource(EngineRoot(engine_root));

        Ok(WinitApp {
            app,
            engine_root,
            render_rate: LimitedRate::new(60.),
        })
    })
}

struct WinitApp {
    app: App,
    engine_root: Entity,
    render_rate: LimitedRate,
}

impl ApplicationHandler for WinitApp {
    fn new_events(&mut self, event_loop: &ActiveEventLoop, _cause: StartCause) {
        if self.render_rate.tick(Instant::now()).output.is_some() {
            self.app
                .use_random(|_: PhantomData<(&ViewportManager, &Viewport)>| {
                    let vmgr = self.engine_root.get::<ViewportManager>();

                    for viewport in vmgr.window_map().values() {
                        viewport.window().request_redraw();
                    }
                });
        }
        event_loop.set_control_flow(ControlFlow::Poll);

        self.app.update();

        self.app.use_random(|_: PhantomData<&ViewportManager>| {
            let vmgr = self.engine_root.get::<ViewportManager>();

            if vmgr.window_map().is_empty() {
                event_loop.exit();
            }
        });
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let _ = event_loop;
    }

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        // Tick input manager
        self.app.use_random(|_: PhantomData<&mut InputManager>| {
            self.engine_root
                .get::<InputManager>()
                .process_window_event(window_id, &event);
        });

        // Handle redraw requests
        if let WindowEvent::RedrawRequested = &event {
            self.app
                .world
                .use_random(|cx| render_app(cx, self.engine_root, window_id));
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        device_id: DeviceId,
        event: DeviceEvent,
    ) {
        self.app.use_random(|_: PhantomData<&mut InputManager>| {
            self.engine_root
                .get::<InputManager>()
                .process_device_event(device_id, &event);
        });
    }
}

#[derive(Debug, Resource)]
pub struct EngineRoot(pub Entity);

fn sys_handle_esc_to_exit(
    mut rand: RandomAccess<(&InputManager, &ViewportManager, &mut Viewport)>,
    engine_root: Res<EngineRoot>,
) {
    rand.provide(|| {
        let inputs = engine_root.0.get::<InputManager>();
        let vmgr = engine_root.0.get::<ViewportManager>();

        for (&window_id, &viewport) in vmgr.window_map() {
            if inputs
                .window(window_id)
                .logical_key(NamedKey::Escape)
                .recently_pressed()
            {
                despawn_entity(viewport.entity());
            }
        }
    });
}

fn sys_reset_input_tracker(
    mut rand: RandomAccess<&mut InputManager>,
    engine_root: Res<EngineRoot>,
) {
    rand.provide(|| {
        engine_root.0.get::<InputManager>().end_tick();
    });
}

#[allow(clippy::type_complexity)]
fn init_engine_root(
    _cx: PhantomData<(
        &mut AssetManager,
        &mut CameraManager,
        &mut GfxContext,
        &mut InputManager,
        &mut Viewport,
        &mut ViewportManager,
        &mut ViewportRenderer,
        &mut VirtualCamera,
    )>,
    event_loop: &ActiveEventLoop,
) -> anyhow::Result<Entity> {
    let engine_root = spawn_entity(());

    // Create main window
    let main_window = Arc::new(
        event_loop.create_window(
            Window::default_attributes()
                .with_title("Crucible")
                .with_visible(false),
        )?,
    );

    // Create asset manager
    engine_root.insert(AssetManager::default());

    // Create camera manager
    engine_root.insert(CameraManager::default());

    // Create graphics singleton
    let (gfx, gfx_surface, _feat_table) =
        futures::executor::block_on(GfxContext::new(main_window.clone(), feat_requires_screen))?;
    let gfx = engine_root.insert(gfx);

    // Register main window viewport
    let viewports = engine_root.insert(ViewportManager::default());

    let gfx_surface_config = gfx_surface.get_default_config(&gfx.adapter, 0, 0).unwrap();

    let main_viewport = spawn_entity(());
    let main_viewport_vp = main_viewport.insert(Viewport::new(
        &gfx,
        main_window,
        Some(gfx_surface),
        gfx_surface_config,
    ));
    main_viewport.insert(ViewportRenderer::new(engine_root));

    viewports.register(main_viewport_vp);

    // Create input manager
    let _input_mgr = engine_root.insert(InputManager::default());

    // Allow game to initialize itself
    world_mut().use_random(|cx| crate::dummy_game::init_engine_root(cx, engine_root));

    // Make main viewport visible
    main_viewport_vp.window().set_visible(true);

    Ok(engine_root)
}

fn render_app(
    _cx: PhantomData<(
        &AssetManager,
        &GfxContext,
        &mut CameraManager,
        &mut Viewport,
        &mut ViewportManager,
        &VirtualCamera,
        ViewportRendererCx,
    )>,
    engine_root: Entity,
    window_id: WindowId,
) {
    let vmgr = engine_root.get::<ViewportManager>();
    let gfx = (*engine_root.get::<GfxContext>()).clone();

    let Some(mut viewport) = vmgr.get_viewport(window_id) else {
        return;
    };

    let Ok(Some(texture)) = viewport.get_current_texture(&gfx) else {
        return;
    };

    let texture_view = texture
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    let mut cmd = gfx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

    viewport
        .entity()
        .get::<ViewportRenderer>()
        .render(&mut cmd, &viewport, &texture_view);

    gfx.queue.submit([cmd.finish()]);

    texture.present();
}
