use arbre::{fetch::*, vtable::*};

pub struct Foo {
    value: u32,
}

impl Comp for Foo {
    type Root = dyn Obj;
}

impl ObjDecl for Foo {
    type Root = dyn Obj;
    const TABLE: VTable<Self, Self::Root> = VTableBuilder::new()
        .expose(identity_field())
        .expose_unsized::<_, dyn FooProxy>(identity_field())
        .into_inner();
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

// Compiles properly as of `rustc 1.55.0-nightly (3e1c75c6e 2021-07-13)`
pub fn fetch_static_static(obj: &Foo) {
    obj.fetch::<Foo>().do_something();
}

// Compiles properly as of `rustc 1.55.0-nightly (3e1c75c6e 2021-07-13)`
pub fn fetch_static_dynamic(obj: &Foo) {
    obj.fetch::<dyn FooProxy>().do_something();
}

// Compiles properly as of `rustc 1.55.0-nightly (3e1c75c6e 2021-07-13)` (still needs associated constants)
pub fn fetch_dynamic_static(obj: &dyn Obj) {
    obj.fetch::<Foo>().do_something();
}

// Compiles properly as of `rustc 1.55.0-nightly (3e1c75c6e 2021-07-13)` (still needs associated constants)
pub fn fetch_dynamic_dynamic(obj: &dyn Obj) {
    obj.fetch::<dyn FooProxy>().do_something();
}
