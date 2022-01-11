//! Core Crucible utilities which can be reused in other engines.

#![feature(alloc_layout_extra)]
#![feature(backtrace)]
#![feature(build_hasher_simple_hash_one)]
#![feature(const_alloc_layout)]
#![feature(decl_macro)]
#![feature(generic_associated_types)]
#![feature(maybe_uninit_write_slice)]
#![feature(never_type)]

pub mod foundation;
pub mod util;
