// === Unstable features === //
// As with any good language hack, Arbre uses a ton of unstable features to just barely make its
// system work.

// Makes macro declarations simpler
#![feature(decl_macro)]

// Allows us to implement custom smart pointers with coercion support
#![feature(coerce_unsized)]
#![feature(unsize)]

// Used to get the raw `u64` representation of `TypeId`, much to the chagrin of the Rust developers.
#![feature(core_intrinsics)]

// Used to get raw TypeIds at compile time.
#![feature(const_type_id)]

// Allows us to display errors for invalid v-tables.
#![feature(const_panic)]

// Allows us to put the Copy bound on `PerfectMap::new()`
#![feature(const_fn_trait_bound)]

// Enables simple deref in PerfectMap
#![feature(const_maybe_uninit_assume_init)]
#![feature(maybe_uninit_ref)]

// PerfectMap takes a *long* time to run so we need to artificially increase the time allotted to it
#![feature(const_eval_limit)]

// Enables the evil magic in `AnyObj`
#![feature(const_raw_ptr_deref)]
#![feature(const_fn_union)]
#![feature(const_mut_refs)]

// For converting a wide pointer to a Sized pointer.
#![feature(ptr_metadata)]

// To make Comp easier to implement
#![feature(associated_type_defaults)]

// === Module declarations === //

mod util;
pub mod obj;
