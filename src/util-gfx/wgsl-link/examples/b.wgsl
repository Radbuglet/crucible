const BAR: u32 = FOO + 2u;

struct Baz {
    faz: i32,
}

struct Foo {
    jg: Baz,
    far: Bar,
    foo: FooNew,
}

fn quux(v: Foo) {
    let kaz = Baz(v.jg.faz);
    whee2(v.foo);
}
