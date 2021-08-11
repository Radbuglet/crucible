// TODO: Use actual errors to allow the build script to fail more gracefully.

use glob::glob;
use shaderc::{Compiler, ShaderKind};
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let mut compiler = Compiler::new().expect("Failed to create shaderc compiler!");

    for entry in glob("src/shader/**/*.vert").expect("Failed to glob `.vert` files") {
        build_shader(
            &mut compiler,
            entry.expect("Failed to glob specific file."),
            ShaderKind::Vertex,
        );
    }

    for entry in glob("src/shader/**/*.frag").expect("Failed to glob `.frag` files") {
        build_shader(
            &mut compiler,
            entry.expect("Failed to glob specific file."),
            ShaderKind::Fragment,
        );
    }
}

fn build_shader(compiler: &mut Compiler, mut path: PathBuf, kind: ShaderKind) {
    // Notify cargo of dependency
    push_rerun_dependency(&path);

    // Build artifact
    let file_name = path.file_name().unwrap().to_str().unwrap();

    let artifact = match compiler.compile_into_spirv(
        &*fs::read_to_string(&path)
            .expect(format!("Failed to read shader source at {:?}", path.as_path()).as_str()),
        kind,
        file_name,
        "main",
        None,
    ) {
        Ok(artifact) => artifact,
        Err(err) => panic!("Failed to build shader {:?}: {}", path.as_path(), err),
    };

    // Update `path` to out path
    let file_name = format!("{}.spv", file_name);
    path.set_file_name(file_name);

    // Write artifact
    fs::write(&path, artifact.as_binary_u8())
        .expect(format!("Failed to write build artifact at {:?}", path.as_path()).as_str());
}

fn push_rerun_dependency(path: &Path) {
    println!("cargo:rerun-if-changed={:?}", path);
}
