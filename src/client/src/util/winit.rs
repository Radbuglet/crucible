use winit::event::Event;
use winit::event_loop::{ControlFlow, EventLoopWindowTarget};

pub type WinitUD = ();
pub type WinitEvent<'a> = Event<'a, WinitUD>;
pub type WinitEventBundle<'a> = (
	&'a WinitEvent<'a>,
	&'a EventLoopWindowTarget<WinitUD>,
	&'a mut ControlFlow,
);
