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
    whee_new(v.foo);
}
