use geode::prelude::*;
use winit::event::Event;
use winit::event_loop::{ControlFlow, EventLoopWindowTarget};

pub type WinitUserdata = ();

#[derive(Debug)]
pub struct WinitEventBundle<'a> {
	pub event: Event<'a, WinitUserdata>,
	pub proxy: &'a EventLoopWindowTarget<WinitUserdata>,
	pub flow: &'a mut ControlFlow,
}

event_trait! {
	pub trait WinitEventHandler::on_winit_event(&self, cx: &mut ObjCx, event: &mut WinitEventBundle);
}
