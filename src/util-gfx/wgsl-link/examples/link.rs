use wgsl_link::linker::{ImportMap, ModuleLinker};

fn main() {
    let mut linker = ModuleLinker::new();
    let mut validator = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    );

    let file_a = linker.link(
        naga::front::wgsl::parse_str(include_str!("a.wgsl")).unwrap(),
        0,
        &ImportMap::new(),
    );

    let b_imports = ImportMap::from_iter([(file_a, "Bar")]);
    let mut b_src = include_str!("b.wgsl").to_string();
    let b_stubs = linker.gen_stubs(&b_imports);
    b_src.push_str(
        &naga::back::wgsl::write_string(
            &b_stubs,
            &validator.validate(&b_stubs).unwrap(),
            naga::back::wgsl::WriterFlags::all(),
        )
        .unwrap(),
    );
    linker.link(naga::front::wgsl::parse_str(&b_src).unwrap(), 0, &b_imports);

    let out = naga::back::wgsl::write_string(
        linker.module(),
        &validator.validate(linker.module()).unwrap(),
        naga::back::wgsl::WriterFlags::all(),
    )
    .unwrap();

    eprintln!("{out}");
}
