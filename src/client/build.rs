use anyhow::Context;
use crucible_core::util::error::{AnyResult, ErrorFormatExt};
use glob::{glob, GlobResult};
use shaderc::{Compiler, ShaderKind};
use std::fs;
use std::path::Path;

fn main() {
	maybe_report_errors(main_inner());
}

fn main_inner() -> AnyResult<()> {
	let mut compiler = Compiler::new().context("Failed to create shaderc compiler!")?;

	for entry in glob("src/**/*.vert").context("Failed to glob `.vert` files")? {
		maybe_report_errors(build_shader(&mut compiler, ShaderKind::Vertex, entry));
	}

	for entry in glob("src/**/*.frag").context("Failed to glob `.frag` files")? {
		maybe_report_errors(build_shader(&mut compiler, ShaderKind::Fragment, entry));
	}

	Ok(())
}

fn build_shader(compiler: &mut Compiler, kind: ShaderKind, entry: GlobResult) -> AnyResult<()> {
	let mut path = entry.context("Failed to get path of a glob file.")?;

	// Notify cargo of dependency
	push_rerun_dependency(&path);

	// Build artifact
	let file_name = path
		.file_name()
		.with_context(|| format!("Failed to isolate shader file name at {:?}", path))?
		.to_str()
		.with_context(|| format!("Failed to decode shader file name at {:?}", path))?;

	let artifact = compiler
		.compile_into_spirv(
			&*fs::read_to_string(&path)
				.with_context(|| format!("Failed to read shader source at {:?}", path.as_path()))?,
			kind,
			file_name,
			"main",
			None,
		)
		.with_context(|| format!("Failed to build shader at {:?}", path.as_path()))?;

	// Update `path` to out path
	let file_name = format!("{}.spv", file_name);
	path.set_file_name(file_name);

	// Write artifact
	fs::write(&path, artifact.as_binary_u8())
		.with_context(|| format!("Failed to write build artifact at {:?}", path.as_path()))?;

	// Finish
	Ok(())
}

fn maybe_report_errors(result: AnyResult<()>) {
	if let Err(err) = result {
		push_warning(format!("{}", err.format_error(false)).as_str());
	}
}

fn push_rerun_dependency(path: &Path) {
	println!("cargo:rerun-if-changed={:?}", path);
}

fn push_warning(text: &str) {
	for line in text.lines() {
		println!("cargo:warning={}", line);
	}
}
