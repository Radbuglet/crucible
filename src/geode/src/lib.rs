#![feature(coerce_unsized)]
#![feature(unsize)]
#![feature(decl_macro)]
#![feature(negative_impls)]
#![feature(ptr_metadata)]

pub mod atomic_ref_cell;
pub mod entity;
pub mod event;
pub mod obj;

mod util;

pub mod prelude {
	// TODO
}
