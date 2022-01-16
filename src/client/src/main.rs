#![allow(dead_code)]
#![feature(decl_macro)]
#![feature(duration_constants)]
#![feature(never_type)]

pub mod engine;
pub mod entry;
pub mod voxel;

fn main() {
	use self::entry::Engine;
	use crucible_core::util::error::ErrorFormatExt;

	if let Err(err) = Engine::start() {
		eprintln!("{}", err.format_error(true));
	}
}
