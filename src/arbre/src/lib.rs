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

// === Module declarations === //

mod util;
pub mod fetch;
pub mod key;
pub mod vtable;

// === Code-gen "tests" === //

#[doc(hidden)]  // FIXME: Move to another crate
pub mod code_gen_tests {
    use super::{
        fetch::*,
        vtable::*,
    };

    pub struct Foo {
        value: u32,
    }

    impl Comp for Foo {
        type Root = dyn Obj;
    }

    impl ObjDecl for Foo {
        type Root = dyn Obj;
        const TABLE: VTable<Self, Self::Root> = {
            let mut table = VTable::new();
            table.expose(Field::identity());
            table.expose_unsized::<Foo, dyn FooProxy>(Field::identity());
            table
        };
    }

    trait FooProxy {
        fn do_something(&self);
    }

    impl FooProxy for Foo {
        fn do_something(&self) {
            if self.value == 0 {
                loop {}
            }
        }
    }

    // Assembled by `rustc 1.55.0-nightly (955b9c0d4 2021-07-12)`, inspected by `cargo-asm 0.1.16`.
    //
    // Command used: `cargo clean && cargo build --release && cargo asm arbre::code_gen_tests::works_correctly`
    //
    // ```
    // arbre::code_gen_tests::works_correctly:
    //  je      .LBB19_1
    //  ret
    // .LBB19_1:
    //  jmp     .LBB19_1
    // ```
    pub fn works_correctly(obj: &Foo) {
        obj.fetch::<dyn FooProxy>().do_something();
    }

    // Assembled by `rustc 1.55.0-nightly (955b9c0d4 2021-07-12)`, inspected by `cargo-asm 0.1.16`.
    //
    // Command used: `cargo clean && cargo build --release && cargo asm arbre::code_gen_tests::broken`
    //
    // ```
    // arbre::code_gen_tests::broken:
    //  add     rcx, qword, ptr, [rip, +, __unnamed_8+584]
    //  rex64   jmp, qword, ptr, [rax, +, 24]
    // ```
    pub fn broken(obj: &Foo) {
        obj.try_fetch::<dyn FooProxy>().unwrap().do_something();
    }
}
