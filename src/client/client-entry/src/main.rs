use std::{process, sync::Arc};

use anyhow::Context;
use bevy_app::App;
use bevy_autoken::{spawn_entity, world_mut, RandomAppExt, RandomEntityExt, RandomWorldExt};
use bevy_ecs::{entity::Entity, world::World};
use main_loop::{
    feat_requires_screen, run_app_with_init, GfxContext, InputManager, Viewport, ViewportManager,
};
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, DeviceId, StartCause, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
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

        // Create main window
        let main_window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title("Crucible")
                    .with_visible(false),
            )?,
        );

        // Create graphics singleton
        let (gfx, gfx_surface, _feat_table) = futures::executor::block_on(GfxContext::new(
            main_window.clone(),
            feat_requires_screen,
        ))?;

        // Register main window viewport
        let mut viewports = ViewportManager::default();

        let gfx_surface_config = gfx_surface.get_default_config(&gfx.adapter, 0, 0).unwrap();
        let main_viewport = app.world.use_random::<&mut Viewport, _>(|| {
            let main_viewport = spawn_entity(()).insert(Viewport::new(
                &gfx,
                main_window,
                Some(gfx_surface),
                gfx_surface_config,
            ));
            viewports.register(main_viewport);
            main_viewport
        });

        // Create input manager
        let input_mgr = InputManager::default();

        // Create singleton
        let engine_root = app
            .world
            .use_random::<(&mut InputManager, &mut ViewportManager, &mut GfxContext), _>(|| {
                let engine_root = world_mut().spawn(()).id();
                engine_root.insert(viewports);
                engine_root.insert(input_mgr);
                engine_root.insert(gfx);
                engine_root
            });

        // Make main viewport visible
        app.world.use_random::<&mut Viewport, _>(|| {
            main_viewport.window().set_visible(true);
        });

        Ok(WinitApp { app, engine_root })
    })
}

struct WinitApp {
    app: App,
    engine_root: Entity,
}

impl ApplicationHandler for WinitApp {
    fn new_events(&mut self, _event_loop: &ActiveEventLoop, _cause: StartCause) {
        self.app
            .world
            .use_random::<(&ViewportManager, &Viewport), _>(|| {
                let vmgr = self.engine_root.get::<ViewportManager>();

                for viewport in vmgr.window_map().values() {
                    viewport.window().request_redraw();
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
        self.app.world.use_random::<&mut InputManager, _>(|| {
            self.engine_root
                .get::<InputManager>()
                .process_window_event(window_id, &event);
        });

        // Handle redraw requests
        if let WindowEvent::RedrawRequested = &event {
            render_app(&mut self.app.world, self.engine_root, window_id);
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        device_id: DeviceId,
        event: DeviceEvent,
    ) {
        self.app.world.use_random::<&mut InputManager, _>(|| {
            self.engine_root
                .get::<InputManager>()
                .process_device_event(device_id, &event);
        });
    }
}

fn render_app(world: &mut World, engine_root: Entity, window_id: WindowId) {
    world.use_random::<(&mut ViewportManager, &mut Viewport, &GfxContext), _>(|| {
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
    });
}
