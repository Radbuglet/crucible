#![feature(ptr_metadata)]
#![feature(unsize)]

pub mod engine;
pub mod game;
pub mod util;

fn main() {
	engine::start_engine();
}
