// === Unstable features === //
// As with any good language hack, Arbre uses a ton of unstable features to just barely make its
// system work.

// TODO: Move `feature` declarations to the places where they're needed.

// The cornerstone of implementing `root` monomorphization on dynamically dispatched traits.
#![feature(arbitrary_self_types)]

// This crate is such a spaghetti pile of unstable features that we need to suppress Rust's call to
// reason.
#![allow(incomplete_features)]

// Makes macro declarations simpler
#![feature(decl_macro)]

// Allows us to implement custom smart pointers with coercion support
#![feature(coerce_unsized)]
#![feature(unsize)]

// Used to get the raw `u64` representation of `TypeId` at compile time, much to the chagrin of the
// Rust developers.
#![feature(core_intrinsics)]
#![feature(const_type_id)]

// Allows us to display errors for invalid v-tables and misuse of internal utilities.
#![feature(const_panic)]

// Allows us to put the `Copy` bound on `PerfectMap::new()`.
#![feature(const_fn_trait_bound)]

// Simplifies the implementation of `PerfectMap` and `ConstVec`.
#![feature(const_maybe_uninit_assume_init)]
#![feature(maybe_uninit_ref)]

// `PerfectMap` takes a *long* time to run so we need to artificially increase the time allotted to it.
#![feature(const_eval_limit)]

// Enables the evil magic of `AnyValue`.
#![feature(const_raw_ptr_deref)]
#![feature(const_fn_union)]
#![feature(const_mut_refs)]

// For converting wide pointers to Sized pointers and vice-versa.
#![feature(ptr_metadata)]

// For an overridable blanket `Comp` implementation.
#![feature(specialization)]

// To implement `fetch_xx_unchecked` without `unchecked_unreachable` hints.
// (we already have so many unstable features, what's the harm in adding a few more?)
#![feature(option_result_unwrap_unchecked)]

// === Module declarations === //

mod util;
pub mod obj;
