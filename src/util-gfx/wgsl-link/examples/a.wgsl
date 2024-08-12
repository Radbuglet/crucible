struct Foo {
    bar: Bar,
}

struct Bar {
    baz: u32,
}

fn whee(v: Foo) {
    let x = v.bar.baz;
}
