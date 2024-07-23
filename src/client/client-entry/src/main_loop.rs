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
use crucible_world::{
    collider::{AabbHolder, AabbStore, BlockColliderDescriptor},
    voxel::{
        sys_clear_dirty_chunk_lists, BlockMaterialRegistry, ChunkVoxelData, WorldChunkCreated,
        WorldVoxelData,
    },
};
use main_loop::{
    feat_requires_screen, run_app_with_init, sys_unregister_dead_viewports, FixedRate, GfxContext,
    InputManager, LimitedRate, Viewport, ViewportManager,
};
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, DeviceId, StartCause, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

use crate::{
    dummy_game::{sys_process_camera_controller, PlayerCameraController},
    render::{
        helpers::{CameraManager, VirtualCamera},
        voxel::{
            sys_attach_mesh_to_visual_chunks, sys_queue_dirty_chunks_for_render, ChunkVoxelMesh,
            MaterialVisualDescriptor, WorldVoxelMesh,
        },
        GlobalRenderer, RenderCx, ViewportRenderer,
    },
};

pub fn main_inner() -> anyhow::Result<()> {
    // Build event loop and start app!
    let event_loop = EventLoop::new().context("failed to create event loop")?;

    run_app_with_init(event_loop, |event_loop| {
        // Create app
        let mut app = App::new();

        app.add_random_component::<AabbHolder>();
        app.add_random_component::<AabbStore>();
        app.add_random_component::<AssetManager>();
        app.add_random_component::<BlockColliderDescriptor>();
        app.add_random_component::<BlockMaterialRegistry>();
        app.add_random_component::<CameraManager>();
        app.add_random_component::<ChunkVoxelData>();
        app.add_random_component::<ChunkVoxelMesh>();
        app.add_random_component::<GfxContext>();
        app.add_random_component::<GlobalRenderer>();
        app.add_random_component::<InputManager>();
        app.add_random_component::<MaterialVisualDescriptor>();
        app.add_random_component::<PlayerCameraController>();
        app.add_random_component::<Viewport>();
        app.add_random_component::<ViewportManager>();
        app.add_random_component::<ViewportRenderer>();
        app.add_random_component::<VirtualCamera>();
        app.add_random_component::<WorldVoxelData>();
        app.add_random_component::<WorldVoxelMesh>();

        app.add_event::<WorldChunkCreated>();

        #[rustfmt::skip]
        app.add_systems(
            Update,
            (
                sys_process_camera_controller,
                sys_attach_mesh_to_visual_chunks,
                sys_queue_dirty_chunks_for_render,
                sys_clear_dirty_chunk_lists,
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
            update_rate: FixedRate::new(60.),
            render_rate: LimitedRate::new(60.),
        })
    })
}

struct WinitApp {
    app: App,
    engine_root: Entity,
    update_rate: FixedRate,
    render_rate: LimitedRate,
}

impl ApplicationHandler for WinitApp {
    fn new_events(&mut self, event_loop: &ActiveEventLoop, _cause: StartCause) {
        // Update and queue render if applicable
        if let Some(times) = self.update_rate.tick(Instant::now()).output {
            for _ in 0..times.get().min(2) {
                self.app.update();
            }
        }

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
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Quit if no windows remain
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
                .world_mut()
                .use_random(|cx| render_app(cx, self.engine_root, window_id));
        }

        // Handle quit requests
        if let WindowEvent::CloseRequested = &event {
            self.app
                .world_mut()
                .use_random(|_: PhantomData<(&ViewportManager, &Viewport)>| {
                    despawn_entity(
                        self.engine_root
                            .get::<ViewportManager>()
                            .get_viewport(window_id)
                            .unwrap()
                            .entity(),
                    );
                });
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
        &mut VirtualCamera,
        (
            &mut AabbStore,
            &mut MaterialVisualDescriptor,
            &mut WorldVoxelMesh,
            &mut WorldVoxelData,
            &mut BlockMaterialRegistry,
        ),
        RenderCx,
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

    // Create voxel stuff
    let registry = engine_root.insert(BlockMaterialRegistry::default());
    engine_root.insert(AabbStore::default());
    engine_root.insert(WorldVoxelData::default());
    engine_root.insert(WorldVoxelMesh::new(registry));

    // Create graphics singleton
    let (gfx, gfx_surface, _feat_table) =
        futures::executor::block_on(GfxContext::new(main_window.clone(), feat_requires_screen))?;
    let gfx = engine_root.insert(gfx);

    // Register main window viewport
    let viewports = engine_root.insert(ViewportManager::default());

    let mut gfx_surface_config = gfx_surface.get_default_config(&gfx.adapter, 0, 0).unwrap();
    gfx_surface_config.format = wgpu::TextureFormat::Bgra8Unorm;

    let main_viewport = spawn_entity(());
    let main_viewport_vp = main_viewport.insert(Viewport::new(
        &gfx,
        main_window,
        Some(gfx_surface),
        gfx_surface_config,
    ));
    engine_root.insert(GlobalRenderer::new(engine_root));
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

#[allow(clippy::type_complexity)]
fn render_app(
    _cx: PhantomData<(
        &AssetManager,
        &BlockMaterialRegistry,
        (&WorldVoxelData, &ChunkVoxelData),
        &GfxContext,
        &MaterialVisualDescriptor,
        &mut CameraManager,
        &mut ChunkVoxelMesh,
        &mut Viewport,
        &mut ViewportManager,
        &mut WorldVoxelMesh,
        &VirtualCamera,
        RenderCx,
    )>,
    engine_root: Entity,
    window_id: WindowId,
) {
    let vmgr = engine_root.get::<ViewportManager>();
    let gfx = (*engine_root.get::<GfxContext>()).clone();
    let mut global_renderer = engine_root.get::<GlobalRenderer>();

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

    global_renderer.render(
        &mut cmd,
        &viewport,
        &mut viewport.entity().get::<ViewportRenderer>(),
        &texture_view,
    );

    gfx.queue.submit([cmd.finish()]);

    texture.present();
}
