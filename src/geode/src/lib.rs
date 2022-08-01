#![allow(clippy::missing_safety_doc)] // TODO: Remove this lint once we have the bandwidth.
#![feature(allocator_api)]
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
			owned::{MaybeOwned, Owned},
			reflect::ReflectType,
			session::{LocalSessionGuard, Session},
		},
		entity::{
			bundle::{
				component_bundle, ComponentBundle, ComponentBundleWithCtor, EntityWith,
				EntityWithRw, MandatoryBundleComp,
			},
			entity::{Entity, EntityGetErrorExt, ExposeUsing},
			event::{
				EventHandler, EventHandlerMut, EventHandlerOnce, EventHandlerOnceMut, Factory,
				FactoryMut,
			},
			key::{proxy_key, typed_key, ProxyKeyType},
		},
	};
}
