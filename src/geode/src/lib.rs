#![feature(coerce_unsized)]
#![feature(decl_macro)]
#![feature(ptr_metadata)]
#![feature(unsize)]

pub mod ecs;
pub mod ecs_next;
pub mod exec;
mod util;

pub mod prelude {
	pub use crate::{
		ecs::{ArchStorage, Entity, MapStorage, World},
		exec::atomic_ref_cell::{AMut, ARef, ARefCell},
		exec::obj::{dyn_key, event_trait, proxy_key, typed_key, Obj, ObjCx, ObjLike},
	};
}

pub use prelude::*;
