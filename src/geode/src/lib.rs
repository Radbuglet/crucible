#![allow(clippy::missing_safety_doc)] // TODO: Remove this lint once we have the bandwidth.
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
			obj::{Lock, Obj, ObjCtorExt, ObjPointee, ObjRw, RawObj},
			owned::Owned,
			reflect::ReflectType,
			session::{LocalSessionGuard, Session},
		},
		entity::{
			bundle::{
				component_bundle, ComponentBundle, ComponentBundleWithCtor, EntityWith,
				EntityWithRw,
			},
			entity::{Entity, EntityGetErrorExt, ExposeUsing},
			event::delegate,
			key::{proxy_key, typed_key, ProxyKeyType},
		},
	};

	pub fn cg_test_alloc(s: Session) -> Owned<Obj<u8>> {
		Obj::new(s, 1u8)
	}
}

// pub use prelude::*;
