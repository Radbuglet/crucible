#![feature(coerce_unsized)]
#![feature(const_type_id)]
#![feature(const_type_name)]
#![feature(decl_macro)]
#![feature(unsize)]
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
