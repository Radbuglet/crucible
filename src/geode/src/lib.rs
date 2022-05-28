// Required by `ARefCell` unsize coercions.
#![feature(coerce_unsized)]
#![feature(unsize)]
// Makes writing safe macros a bit easier.
#![feature(decl_macro)]
// Temporary lint suppression. Disable once we begin the process of releasing the crate.
#![allow(clippy::missing_safety_doc)]

// hibitset doesn't even support platforms with 16 bit `usize`s so there's no point bothering. Also,
// we're not no-std.
// FIXME: We still might want to use proper arithmetic operations to ensure that our logic could
//  theoretically still work on a 16 bit machine, however unlikely that may be. It's just a better
//  practice that helps us avoid other bit-width related foot-guns later on.
#[cfg(target_pointer_width = "16")]
compile_error!(
	"Geode does not support platforms with a pointer width of 16 bits. (Why the heck are you \
                making a game on an embedded platform in the first place?)"
);

pub mod ecs;
pub mod exec;
mod util;

pub mod prelude {
	pub use crate::{
		ecs::prelude::*,
		exec::atomic_ref_cell::{AMut, ARef, ARefCell},
		exec::event::event_trait,
		exec::key::{dyn_key, proxy_key, typed_key, TypedKey},
		exec::obj::{Obj, ObjCx, ObjExt, SendObj, StObj},
	};
}
