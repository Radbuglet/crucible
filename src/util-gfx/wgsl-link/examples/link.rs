use wgsl_link::module::linker::{ImportStubs, ModuleLinker};

fn main() {
    let mut linker = ModuleLinker::new();
    let mut validator = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    );

    let file_a = linker.link(
        naga::front::wgsl::parse_str(include_str!("a.wgsl")).unwrap(),
        &ImportStubs::empty(),
        0,
    );

    let b_stubs = linker.gen_stubs([
        (file_a, "whee", Some("whee_new")),
        (file_a, "Bar", None),
        (file_a, "Foo", Some("FooNew")),
        (file_a, "FOO", None),
    ]);

    let mut b_src = include_str!("b.wgsl").to_string();
    b_src.push_str("\n// === Stubs === //\n\n");
    b_src.push_str(
        &naga::back::wgsl::write_string(
            b_stubs.module(),
            &validator.validate(b_stubs.module()).unwrap(),
            naga::back::wgsl::WriterFlags::all(),
        )
        .unwrap(),
    );
    eprintln!("{b_src}");
    linker.link(naga::front::wgsl::parse_str(&b_src).unwrap(), &b_stubs, 0);

    let out = naga::back::wgsl::write_string(
        linker.full_module(),
        &validator.validate(linker.full_module()).unwrap(),
        naga::back::wgsl::WriterFlags::all(),
    )
    .unwrap();

    eprintln!("// === Linked Output === //\n\n{out}");
}
