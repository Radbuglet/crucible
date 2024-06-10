use std::{marker::PhantomData, process, sync::Arc};

use anyhow::Context;
use bevy_app::{App, Update};
use bevy_autoken::{
    despawn_entity, spawn_entity, RandomAccess, RandomAppExt, RandomEntityExt, RandomWorldExt,
};
use bevy_ecs::{
    entity::Entity,
    schedule::IntoSystemConfigs,
    system::{Res, Resource},
};
use main_loop::{
    feat_requires_screen, run_app_with_init, sys_unregister_dead_viewports, GfxContext,
    InputManager, Viewport, ViewportManager,
};
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, DeviceId, StartCause, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::NamedKey,
    window::{Window, WindowId},
};

fn main() {
    // Install early (infallible) services
    color_backtrace::install();
    tracing_subscriber::fmt::init();

    tracing::info!("Hello!");

    // Run main (fallible) app logic
    if let Err(err) = main_inner() {
        tracing::error!("Fatal error ocurred during engine startup:\n{err:?}");
        process::exit(1);
    }

    tracing::info!("Goodbye!");
}

fn main_inner() -> anyhow::Result<()> {
    // Build event loop and start app!
    let event_loop = EventLoop::new().context("failed to create event loop")?;

    run_app_with_init(event_loop, |event_loop| {
        // Create app
        let mut app = App::new();

        app.add_random_component::<GfxContext>();
        app.add_random_component::<InputManager>();
        app.add_random_component::<Viewport>();
        app.add_random_component::<ViewportManager>();

        app.add_systems(
            Update,
            (sys_handle_esc_to_exit, sys_unregister_dead_viewports).chain(),
        );

        // Initialize engine root
        let engine_root = app.use_random(
            |_: PhantomData<(
                &mut ViewportManager,
                &mut Viewport,
                &mut GfxContext,
                &mut InputManager,
            )>| {
                let engine_root = spawn_entity(());

                // Create main window
                let main_window = Arc::new(
                    event_loop.create_window(
                        Window::default_attributes()
                            .with_title("Crucible")
                            .with_visible(false),
                    )?,
                );

                // Create graphics singleton
                let (gfx, gfx_surface, _feat_table) = futures::executor::block_on(
                    GfxContext::new(main_window.clone(), feat_requires_screen),
                )?;
                let gfx = engine_root.insert(gfx);

                // Register main window viewport
                let viewports = engine_root.insert(ViewportManager::default());

                let gfx_surface_config =
                    gfx_surface.get_default_config(&gfx.adapter, 0, 0).unwrap();

                let main_viewport = spawn_entity(()).insert(Viewport::new(
                    &gfx,
                    main_window,
                    Some(gfx_surface),
                    gfx_surface_config,
                ));
                viewports.register(main_viewport);

                // Create input manager
                let _input_mgr = engine_root.insert(InputManager::default());

                // Make main viewport visible
                main_viewport.window().set_visible(true);

                Ok::<_, anyhow::Error>(engine_root)
            },
        )?;

        app.insert_resource(EngineRoot(engine_root));

        Ok(WinitApp { app, engine_root })
    })
}

struct WinitApp {
    app: App,
    engine_root: Entity,
}

impl ApplicationHandler for WinitApp {
    fn new_events(&mut self, event_loop: &ActiveEventLoop, _cause: StartCause) {
        self.app
            .use_random(|_: PhantomData<(&ViewportManager, &Viewport)>| {
                let vmgr = self.engine_root.get::<ViewportManager>();

                for viewport in vmgr.window_map().values() {
                    viewport.window().request_redraw();
                }
            });

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

fn render_app(
    _ty: PhantomData<(&mut ViewportManager, &mut Viewport, &GfxContext)>,
    engine_root: Entity,
    window_id: WindowId,
) {
    let vmgr = engine_root.get::<ViewportManager>();
    let gfx = engine_root.get::<GfxContext>();

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

    let pass = cmd.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: None,
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &texture_view,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color {
                    r: 0.1,
                    b: 0.1,
                    g: 0.1,
                    a: 1.0,
                }),
                store: wgpu::StoreOp::Store,
            },
            resolve_target: None,
        })],
        depth_stencil_attachment: None,
        occlusion_query_set: None,
        timestamp_writes: None,
    });
    drop(pass);

    gfx.queue.submit([cmd.finish()]);

    texture.present();
}
