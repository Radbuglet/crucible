pub mod gfx;
pub mod input;

pub type WinitEvent<'a> = winit::event::Event<'a, ()>;
pub type WindowPosPx = winit::dpi::PhysicalPosition<f64>;
pub type WindowSizePx = winit::dpi::PhysicalSize<u32>;
