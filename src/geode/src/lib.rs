#![feature(const_type_id)]
#![feature(const_type_name)]
#![feature(decl_macro)]
#![feature(unsize)]
#![feature(ptr_metadata)]

mod internals;
mod util;

pub mod debug;
pub mod entity;
pub mod event;
pub mod key;
pub mod obj;
pub mod session;

pub mod prelude {
    pub use crate::{
        debug::NoLabel,
        entity::{},
        event::event_trait,
        key::typed_key,
        obj::{Lock, LockToken, ObjCtorExt, RawObj, Obj, ObjRw},
        session::Session,
    };
}

pub use prelude::*;
