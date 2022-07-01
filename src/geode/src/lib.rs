#![feature(const_type_id)]
#![feature(const_type_name)]
#![feature(decl_macro)]
#![feature(unsize)]
#![feature(ptr_metadata)]
#![feature(thread_local)]

mod util;

pub mod core;
pub mod entity;

pub mod prelude {
	pub use crate::{
		core::{
			debug::NoLabel,
			obj::{Lock, Obj, ObjCtorExt, ObjRw, RawObj},
			owned::Owned,
			reflect::ReflectType,
			session::{LocalSessionGuard, Session},
		},
		entity::{
			entity::{Entity, ExposeUsing},
			event::event_trait,
			key::{proxy_key, typed_key, ProxyKeyType},
		},
	};
}

// pub use prelude::*;
