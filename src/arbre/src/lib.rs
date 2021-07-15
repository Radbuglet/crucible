// === Unstable features === //
// As with any good language hack, Arbre uses a ton of unstable features to just barely make its
// system work.

// So that function pointer variance works in const-fn
#![feature(const_fn_fn_ptr_basics)]

// Makes macro declarations simpler
#![feature(decl_macro)]

// Allows us to implement coercion in `Field`.
#![feature(unsize)]

// Used to get the raw `u64` representation of `TypeId` at compile time, much to the chagrin of the
// Rust developers.
#![feature(core_intrinsics)]
#![feature(const_type_id)]

// Allows us to display errors for compile time constructs.
#![feature(const_panic)]

// Simplifies the implementation of `ConstVec` and `RawVTable`.
#![feature(const_maybe_uninit_assume_init)]

// Allows us to create slices to `ConstVec`'s contents.
#![feature(const_slice_from_raw_parts)]
#![feature(const_ptr_offset)]

// Allows us to add the `T: Copy` constraint in `ConstVec`
#![feature(const_fn_trait_bound)]

// `VTable::build` takes a *long* time to run so we need to artificially increase the time allotted to it.
#![feature(const_eval_limit)]

// Enables the evil magic of `AnyValue`.
#![feature(const_raw_ptr_deref)]
#![feature(const_fn_union)]
#![feature(const_mut_refs)]

// Allows us to calculate `Field` byte offsets
#![feature(const_ptr_offset_from)]

// For converting wide pointers to Sized pointers and vice-versa.
#![feature(ptr_metadata)]

// To implement `fetch_xx_unchecked` without `unchecked_unreachable` hints.
// (we already have so many unstable features, what's the harm in adding a few more?)
#![feature(option_result_unwrap_unchecked)]

// To fix a weird code-gen issue in `AnyValue`.
#![feature(transparent_unions)]

// === Module declarations === //

mod util;
pub mod fetch;
pub mod key;
pub mod vtable;

