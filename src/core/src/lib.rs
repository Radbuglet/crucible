#![allow(clippy::missing_safety_doc)] // TODO: Unsuppress this lint once we have the bandwidth.
#![feature(decl_macro)]

pub mod debug;
pub mod lang;
pub mod mem;
pub mod prim;

pub mod prelude {
	pub use crate::{
		lang::std_traits::{MutMarker, RefMarker},
		prim::{
			entity::{Demand, Entity, Provider, ProviderExt},
			lock::{CompCell, DynSession, Session, StaticSession},
		},
	};
}
