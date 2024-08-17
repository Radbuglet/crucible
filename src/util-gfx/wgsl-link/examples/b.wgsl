//#use whee as whee2, Bar, Foo as FooNew, FOO as FOOD in "a.wgsl"

const BAR: u32 = FOOD + 2u;

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
