use wgsl_link::linker::ModuleLinker;

fn main() {
    let a = naga::front::wgsl::parse_str(include_str!("a.wgsl")).unwrap();
    let b = naga::front::wgsl::parse_str(include_str!("b.wgsl")).unwrap();

    let mut linker = ModuleLinker::new();
    let mut validator = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    );
    linker.link(a, 0, |_| None);
    linker.link(b, 0, |_| None);

    let out = naga::back::wgsl::write_string(
        linker.module(),
        &validator.validate(linker.module()).unwrap(),
        naga::back::wgsl::WriterFlags::all(),
    )
    .unwrap();

    eprintln!("{out}");
}
