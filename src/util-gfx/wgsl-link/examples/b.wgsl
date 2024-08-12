struct Baz {
    faz: i32,
}

struct Foo {
    jg: Baz,
    far: Bar,
}

fn quux(v: Foo) {
    let kaz = Baz(v.jg.faz);
}
