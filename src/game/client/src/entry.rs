use anyhow::Context;
use crucible_foundation_client::engine::io::{
	gfx::{feat_requires_power_pref, feat_requires_screen, CompatQueryInfo, GfxContext, Judgement},
	main_loop::{MainLoop, MainLoopHandler, WinitEventProxy, WinitUserdata},
};
use winit::{
	dpi::LogicalSize,
	event_loop::{EventLoop, EventLoopBuilder},
	window::{WindowBuilder, WindowId},
};

#[derive(Debug)]
struct MyMainLoopHandler {}

impl MainLoopHandler for MyMainLoopHandler {
	fn on_update(&mut self, main_loop: &mut MainLoop, winit: &WinitEventProxy) {
		todo!()
	}

	fn on_render(
		&mut self,
		main_loop: &mut MainLoop,
		winit: &WinitEventProxy,
		window_id: WindowId,
	) {
		todo!()
	}
}

pub fn main_inner() -> anyhow::Result<()> {
	// Create the event loop
	let event_loop: EventLoop<WinitUserdata> = EventLoopBuilder::with_user_event().build();

	// Create the main window
	let main_window = WindowBuilder::new()
		.with_title("Crucible")
		.with_visible(false)
		.with_inner_size(LogicalSize::new(1920, 1080))
		.build(&event_loop)
		.context("failed to create main window")?;

	// Create the graphics context
	let gfx = GfxContext::new(&main_window, |info: &mut CompatQueryInfo| {
		Judgement::new_ok("Adapter is suitable")
			.sub(feat_requires_screen(info).0)
			.sub(feat_requires_power_pref(wgpu::PowerPreference::HighPerformance)(info).0)
			.with_table(())
	});

	// Create the handler and start the main loop
	let handler = MyMainLoopHandler {};

	MainLoop::start(event_loop, handler);
}
