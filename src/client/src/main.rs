#![feature(backtrace)]
#![feature(decl_macro)]
#![feature(never_type)]

use crate::util::error::ErrorFormatExt;

mod render;
mod util;

fn main() {
	if let Err(err) = main_inner() {
		eprintln!("{}", err.format_error(true));
	}
}

fn main_inner() -> anyhow::Result<!> {
	todo!()
}
