pub mod engine;
pub mod util;

fn main() {
	if let Err(err) = engine::entry::main_inner() {
		eprintln!("{:#?}", err);
		std::process::exit(1);
	}
}
