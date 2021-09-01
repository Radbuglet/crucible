#![feature(backtrace)]
#![feature(decl_macro)]
#![feature(never_type)]

use crate::render::core::GfxManager;
use crate::util::error::ErrorFormatExt;
use crate::util::winit::WinitEventBundle;
use winit::dpi::LogicalSize;
use winit::event_loop::EventLoop;
use winit::window::WindowBuilder;

mod render;
mod util;

fn main() {
	if let Err(err) = main_inner() {
		eprintln!("{}", err.format_error(true));
	}
}

fn main_inner() -> anyhow::Result<!> {
	// Create window
	let event_loop = EventLoop::new();
	let window = WindowBuilder::new()
		.with_title("Crucible")
		.with_inner_size(LogicalSize::new(1920, 1080))
		.with_resizable(true)
		.with_visible(false)
		.build(&event_loop)?;

	// Setup gfx singleton
	let (mut gfx, window_id) = GfxManager::new_with_window(window)?;

	// Start engine
	gfx.get_window(&window_id).set_visible(true);
	event_loop.run(move |ev, proxy, flow| {
		let ev: WinitEventBundle = (&ev, proxy, flow);
		gfx.handle_ev(ev);
	})
}
