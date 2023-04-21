#![allow(dead_code)]

mod engine;
mod game;

fn main() {
	env_logger::init();

	if let Err(err) = engine::entry::main() {
		eprintln!("{:#?}", err);
		std::process::exit(1);
	}
}
