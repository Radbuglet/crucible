#![feature(const_type_id)]
#![feature(const_type_name)]
#![feature(decl_macro)]
#![feature(unsize)]
#![feature(ptr_metadata)]

mod util;

pub mod core;
pub mod entity;

pub mod prelude {
	pub use crate::{
		core::{
			debug::NoLabel,
			obj::{Lock, LockToken, Obj, ObjCtorExt, ObjRw, RawObj},
			owned::Owned,
			reflect::TypeMeta,
			session::Session,
		},
		entity::{
			entity::{Entity, ExposeAs},
			event::event_trait,
			key::typed_key,
		},
	};
}

pub use prelude::*;
