#![feature(backtrace)]
#![feature(decl_macro)]
#![feature(never_type)]

use crate::render::core::{Viewport, ViewportConfig, VkContext};
use crate::render::util::wrap::VkSurface;
use crate::render::vk_prelude::*;
use crate::util::error::ErrorFormatExt;
use winit::dpi::LogicalSize;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;

mod render;
mod util;

fn main() {
	if let Err(err) = main_inner() {
		eprintln!("{}", err.format_error(true));
	}
}

fn main_inner() -> anyhow::Result<!> {
	unsafe {
		// Set up windowing system
		let event_loop = EventLoop::new();
		let main_window = WindowBuilder::new()
			.with_title("Crucible")
			.with_visible(false)
			.with_inner_size(LogicalSize::new(1920, 1080))
			.build(&event_loop)?;

		// Setup Vulkan
		let (vk_cx, main_surface) = VkContext::new(&main_window)?;
		let main_surface = VkSurface::new(&vk_cx, main_surface)?;
		let viewport = Viewport::from_parts(
			&vk_cx,
			ViewportConfig {
				compositor_blend: vk::CompositeAlphaFlagsKHR::OPAQUE_KHR,
				compositor_can_clip: false,
				present_mode_prefs: vec![],
			},
			main_window,
			main_surface,
		)?;

		// Start main loop
		viewport.window().set_visible(true);
		event_loop.run(move |event, proxy, flow| {
			if let Event::WindowEvent {
				event: WindowEvent::CloseRequested,
				..
			} = &event
			{
				*flow = ControlFlow::Exit;
			}
		});
	}
}
