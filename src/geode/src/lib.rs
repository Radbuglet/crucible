#![feature(coerce_unsized)]
#![feature(unsize)]
#![feature(decl_macro)]
#![feature(ptr_metadata)]

pub mod oop;
mod util;

pub mod prelude {
	pub use crate::{
		oop::atomic_ref_cell::{AMut, ARef, ARefCell},
		oop::event::event_trait,
		oop::key::{dyn_key, proxy_key, typed_key, TypedKey},
		oop::obj::{
			cx::{ObjCx, SendObjCx, StObjCx},
			obj::{Obj, SendObj, StObj},
			raw::{ObjExt, RawObj},
		},
	};
}
