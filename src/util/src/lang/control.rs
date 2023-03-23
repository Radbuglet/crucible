#[doc(hidden)]
pub mod macro_internals {
	pub use std::unreachable;
}

#[macro_export]
macro_rules! match_unwrap {
	($pat:pat = $expr:expr) => {
		let $pat = $expr else { $crate::lang::control::macro_internals::unreachable!() };
	};
}
