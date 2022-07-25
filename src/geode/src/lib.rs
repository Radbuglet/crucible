#![allow(clippy::missing_safety_doc)] // TODO: Remove this lint once we have the bandwidth.
#![feature(allocator_api)]
#![feature(const_type_id)]
#![feature(const_type_name)]
#![feature(decl_macro)]
#![feature(unsize)]
#![feature(ptr_metadata)]
#![feature(thread_local)]

mod util;

pub mod container;
pub mod core;
pub mod entity;

pub mod prelude {
	pub use crate::{
		container::{cell::CellExt, signal::Signal},
		core::{
			debug::NoLabel,
			obj::{Lock, Obj, ObjCtorExt, ObjPointee, ObjRw, RawObj},
			owned::{Owned, OwnedOrWeak},
			reflect::ReflectType,
			session::{LocalSessionGuard, Session},
		},
		entity::{
			bundle::{
				component_bundle, ComponentBundle, ComponentBundleWithCtor, EntityWith,
				EntityWithRw,
			},
			entity::{Entity, EntityGetErrorExt, ExposeUsing},
			event::{
				EventHandler, EventHandlerMut, EventHandlerOnce, EventHandlerOnceMut, Factory,
				FactoryMut,
			},
			key::{proxy_key, typed_key, ProxyKeyType},
		},
	};

	pub fn cg_test_alloc(s: Session) -> Owned<Obj<u8>> {
		Obj::new(s, 1u8)
	}
}

// pub use prelude::*;

pub fn obj_deref(s: prelude::Session, obj: prelude::Obj<u32>) -> u32 {
	*obj.get(s)
}
