use wgsl_link::driver::{build_script::run_build_script, session::Wgsl};

fn main() {
    run_build_script(Wgsl::default(), [("src/render/shaders", "shaders")]);
}
