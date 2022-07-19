#![feature(never_type)]

pub mod engine;
pub mod game;
pub mod util;

fn main() {
	env_logger::init();

	if let Err(err) = engine::startup::main_inner() {
		eprintln!("{:#?}", err);
		std::process::exit(1);
	}
}
