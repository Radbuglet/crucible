use winit::event::Event;
use winit::event_loop::{ControlFlow, EventLoopWindowTarget};

pub type WinitUserdata = ();

#[derive(Debug)]
pub struct WinitEventBundle<'a> {
	pub event: Event<'a, WinitUserdata>,
	pub proxy: &'a EventLoopWindowTarget<WinitUserdata>,
	pub flow: &'a mut ControlFlow,
}
