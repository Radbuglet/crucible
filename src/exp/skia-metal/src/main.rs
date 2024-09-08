use std::{env, ffi::CStr};

use metal_rs::{CaptureDescriptor, MTLCaptureDescriptor, MTLCaptureDestination, MTLCaptureManager};

extern "C" {
    // Needed to catch exceptions thrown from `MTLCaptureManager::startCaptureWithDescriptor`.
    fn skia_metal_start_capture_with_catch(
        manager: *const MTLCaptureManager,
        descriptor: *const MTLCaptureDescriptor,
    ) -> bool;

    // Needed because the bindings for `metal::CaptureDescriptor::set_output_url` don't write the
    // property with the appropriate `NSURL` type.
    fn skia_metal_set_output_url(descriptor: *const MTLCaptureDescriptor, url: *const libc::c_char);
}

fn main() {
    use core_graphics_types::geometry::CGSize;
    use foreign_types_shared::ForeignTypeRef;
    use objc::rc::autoreleasepool;
    use winit::{
        application::ApplicationHandler,
        event::WindowEvent,
        event_loop::{ActiveEventLoop, EventLoop},
        window::WindowId,
    };

    use skia_safe::{
        gpu::{self, backend_render_targets, mtl, SurfaceOrigin},
        scalar, ColorType,
    };

    let event_loop = EventLoop::new().expect("Failed to create event loop");

    struct Application {
        context: Option<window::Context>,
        should_capture: bool,
    }

    let mut application = Application {
        context: None,
        should_capture: false,
    };

    impl ApplicationHandler for Application {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            assert!(self.context.is_none());
            self.context = Some(window::Context::new(event_loop))
        }

        fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
            self.context.as_mut().unwrap().stop();
        }

        fn window_event(
            &mut self,
            event_loop: &ActiveEventLoop,
            _window_id: WindowId,
            event: WindowEvent,
        ) {
            let context = &mut self.context.as_mut().unwrap();
            match event {
                WindowEvent::CloseRequested => event_loop.exit(),
                WindowEvent::KeyboardInput { event, .. } => {
                    if event.logical_key.to_text() == Some(" ") && event.state.is_pressed() {
                        eprintln!("Queueing capture!");
                        self.should_capture = true;
                        context.window.request_redraw();
                    }
                }
                WindowEvent::Resized(size) => {
                    context
                        .metal_layer
                        .set_drawable_size(CGSize::new(size.width as f64, size.height as f64));
                    context.window.request_redraw()
                }
                WindowEvent::RedrawRequested => {
                    if let Some(drawable) = context.metal_layer.next_drawable() {
                        let (drawable_width, drawable_height) = {
                            let size = context.metal_layer.drawable_size();
                            (size.width as scalar, size.height as scalar)
                        };

                        if self.should_capture {
                            eprintln!("Starting capture...");

                            let capture_descriptor = CaptureDescriptor::new();
                            capture_descriptor.set_capture_device(&context.device);
                            capture_descriptor
                                .set_destination(MTLCaptureDestination::GpuTraceDocument);

                            unsafe {
                                skia_metal_set_output_url(
                                    capture_descriptor.as_ptr(),
                                    CStr::from_bytes_until_nul(
                                        format!(
                                            "file:///{}/trace.gputrace\0",
                                            env::current_dir().unwrap().display(),
                                        )
                                        .as_bytes(),
                                    )
                                    .unwrap()
                                    .as_ptr(),
                                );
                            }

                            let res = unsafe {
                                skia_metal_start_capture_with_catch(
                                    context.capture_manager.as_ptr(),
                                    capture_descriptor.as_ptr(),
                                )
                            };

                            if !res {
                                self.should_capture = false;
                                eprintln!("Failed to capture");
                            }
                        }

                        let mut surface = unsafe {
                            let texture_info =
                                mtl::TextureInfo::new(drawable.texture().as_ptr() as mtl::Handle);

                            let backend_render_target = backend_render_targets::make_mtl(
                                (drawable_width as i32, drawable_height as i32),
                                &texture_info,
                            );

                            gpu::surfaces::wrap_backend_render_target(
                                &mut context.skia,
                                &backend_render_target,
                                SurfaceOrigin::TopLeft,
                                ColorType::BGRA8888,
                                None,
                                None,
                            )
                            .unwrap()
                        };

                        window::draw(surface.canvas());

                        context.skia.flush_and_submit();
                        drop(surface);

                        let command_buffer = context.command_queue.new_command_buffer();
                        command_buffer.present_drawable(drawable);
                        command_buffer.commit();

                        if self.should_capture {
                            eprintln!("Ending capture...");
                            context.capture_manager.stop_capture();
                            self.should_capture = false;
                        }
                    }
                }
                _ => (),
            }
        }
    }

    // As of winit 0.30.0, this is crashing on exit:
    // https://github.com/rust-windowing/winit/issues/3668
    autoreleasepool(|| {
        event_loop.run_app(&mut application).expect("run() failed");
    })
}

mod window {

    use cocoa::{appkit::NSView, base::id as cocoa_id};
    use core_graphics_types::geometry::CGSize;
    use foreign_types_shared::ForeignType;
    use metal_rs::{
        CaptureManager, CaptureManagerRef, CommandQueue, Device, MTLCaptureDestination,
        MTLPixelFormat, MetalLayer,
    };
    use objc::runtime::YES;
    use skia_safe::{
        gpu::{self, mtl, DirectContext},
        Canvas, Color4f, Paint, Rect,
    };
    use typed_glam::glam::Vec2;
    use winit::{
        dpi::{LogicalSize, Size},
        event_loop::ActiveEventLoop,
        raw_window_handle::HasWindowHandle,
        window::{Window, WindowAttributes},
    };

    pub struct Context {
        pub window: Window,
        pub device: Device,
        pub metal_layer: MetalLayer,
        pub command_queue: CommandQueue,
        pub capture_manager: &'static CaptureManagerRef,
        pub skia: DirectContext,
    }

    impl Context {
        pub fn new(event_loop: &ActiveEventLoop) -> Self {
            let size = LogicalSize::new(800, 600);
            let mut window_attributes = WindowAttributes::default();
            window_attributes.inner_size = Some(Size::new(size));
            window_attributes.title = "Skia Benchmark".to_string();

            let window = event_loop
                .create_window(window_attributes)
                .expect("Failed to create Window");

            let window_handle = window
                .window_handle()
                .expect("Failed to retrieve a window handle");

            let raw_window_handle = window_handle.as_raw();

            let device = Device::system_default().expect("no device found");

            let metal_layer = {
                let draw_size = window.inner_size();
                let layer = MetalLayer::new();
                layer.set_device(&device);
                layer.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
                layer.set_presents_with_transaction(false);
                // Disabling this option allows Skia's Blend Mode to work.
                // More about: https://developer.apple.com/documentation/quartzcore/cametallayer/1478168-framebufferonly
                layer.set_framebuffer_only(false);

                unsafe {
                    let view = match raw_window_handle {
                        raw_window_handle::RawWindowHandle::AppKit(appkit) => {
                            appkit.ns_view.as_ptr()
                        }
                        _ => panic!("Wrong window handle type"),
                    } as cocoa_id;
                    view.setWantsLayer(YES);
                    view.setLayer(layer.as_ref() as *const _ as _);
                }
                layer.set_drawable_size(CGSize::new(
                    draw_size.width as f64,
                    draw_size.height as f64,
                ));
                layer
            };

            let command_queue = device.new_command_queue();

            let backend = unsafe {
                mtl::BackendContext::new(
                    device.as_ptr() as mtl::Handle,
                    command_queue.as_ptr() as mtl::Handle,
                )
            };

            let skia_context = gpu::direct_contexts::make_metal(&backend, None).unwrap();

            let capture_manager = CaptureManager::shared();
            assert!(capture_manager.supports_destination(MTLCaptureDestination::GpuTraceDocument));

            Self {
                window,
                device,
                metal_layer,
                command_queue,
                capture_manager,
                skia: skia_context,
            }
        }

        pub fn stop(&mut self) {
            CaptureManager::shared().stop_capture();
        }
    }

    /// Renders a rectangle that occupies exactly half of the canvas
    pub fn draw(canvas: &Canvas) {
        let canvas_size = skia_safe::Size::from(canvas.base_layer_size());

        canvas.clear(Color4f::new(1.0, 1.0, 1.0, 1.0));

        let mut path = skia_safe::Path::new();
        path.add_circle(
            (canvas_size.width / 2., canvas_size.height / 2.),
            100.,
            None,
        );
        canvas.clip_path(&path, None, Some(true));

        fastrand::seed(4);

        for _ in 0..100_000 {
            let point_a = Vec2::new(
                fastrand::f32() * canvas_size.width,
                fastrand::f32() * canvas_size.height,
            );
            let point_b = Vec2::new(
                fastrand::f32() * canvas_size.width,
                fastrand::f32() * canvas_size.height,
            );

            let min = point_a.min(point_b);
            let max = point_a.max(point_b);
            let size = (max - min).min(Vec2::splat(100.));

            let rect = Rect::from_xywh(min.x, min.y, size.x, size.y);
            canvas.draw_rect(
                rect,
                &Paint::new(
                    Color4f::new(fastrand::f32(), fastrand::f32(), fastrand::f32(), 1.0),
                    None,
                ),
            );
        }
    }
}
