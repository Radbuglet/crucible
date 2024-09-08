fn main() {
    println!("cargo::rerun-if-changed=ffi");
    cc::Build::new()
        .file("ffi/exceptionTrampoline.m")
        .compile("skia_metal_ffi")
}
