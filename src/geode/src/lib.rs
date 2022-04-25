#![feature(coerce_unsized)]
#![feature(decl_macro)]
#![feature(ptr_metadata)]
#![feature(unsize)]

// hibitset doesn't even support platforms with 16 bit `usize`s so there's no point bothering. Also,
// we're not no-std.
//
// FIXME: We still might want to use proper arithmetic operations to ensure that our logic could
//  theoretically still work on a 16 bit machine, however unlikely that may be. It's just a better
//  practice that helps us avoid other bit-width related foot-guns later on.
#[cfg(target_pointer_width = "16")]
compile_error!("Geode does not support platforms with a pointer width of 16 bits. (Why the heck are you making a game on an embedded platform in the first place?)");

pub mod ecs;
pub mod ecs_next;
pub mod exec;
mod util;

pub mod prelude {
	pub use crate::{
		ecs::{ArchStorage, Entity, MapStorage, World},
		exec::atomic_ref_cell::{AMut, ARef, ARefCell},
		exec::obj::{
			dyn_key, event_trait, proxy_key, typed_key, Obj, ObjCx, ObjLike, SendObj, StObj,
		},
	};
}

pub use prelude::*;
