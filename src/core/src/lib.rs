#![allow(clippy::missing_safety_doc)] // TODO: Remove this lint once we have the bandwidth.
#![feature(const_type_id)]
#![feature(const_type_name)]
#![feature(decl_macro)]
#![feature(never_type)]

mod ext;
mod util;

pub use ext::*;
pub use util::*;
