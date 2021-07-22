// === Unstable features === //
// As with any good language hack, Arbre uses a ton of unstable features to just barely make its
// system work.

// So that function pointer variance works in const-fn
#![feature(const_fn_fn_ptr_basics)]

// Makes macro declarations simpler
#![feature(decl_macro)]

// Used to get the raw `u64` representation of `TypeId` at compile time, much to the chagrin of the
// Rust developers.
#![feature(core_intrinsics)]
#![feature(const_type_id)]

// Allows us to display errors for compile time constructs.
#![feature(const_panic)]

// Simplifies the implementation of `ConstVec` and `RawVTable`.
#![feature(const_maybe_uninit_assume_init)]

// Allows us to add the `T: Copy` constraint in `ConstVec`
#![feature(const_fn_trait_bound)]

// Enables the evil magic of `AnyValue`.
#![feature(const_fn_union)]
#![feature(const_mut_refs)]
#![feature(const_ptr_write)]

// Allows us to query the value of an `AnyValue`.
#![feature(const_raw_ptr_deref)]
#![feature(const_ptr_read)]

// Allows us to calculate `Field` byte offsets
#![feature(const_ptr_offset_from)]

// To fix a weird code-gen issue in `AnyValue`.
#![feature(transparent_unions)]

// For converting wide pointers to Sized pointers and vice-versa.
#![feature(ptr_metadata)]
#![feature(unsize)]
#![feature(coerce_unsized)]

// To implement `fetch_xx_unchecked` without `unchecked_unreachable` hints.
// (we already have so many unstable features, what's the harm in adding a few more?)
#![feature(option_result_unwrap_unchecked)]

// === Module declarations === //

mod util;
pub mod fetch;
pub mod key;
pub mod vtable;

pub mod prelude {
    pub use crate::{
        fetch::{Obj, ObjExt, ObjDecl, CompRef},
        key::{Key, new_key},
    };
}
pub use prelude::*;
