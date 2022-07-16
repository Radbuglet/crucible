mod util;
mod vec_derive;

fn main() -> anyhow::Result<()> {
	use anyhow::{bail, Context};
	use clap::{Parser, Subcommand};
	// use clipboard::ClipboardProvider;
	use genco::prelude::*;
	use std::{collections::HashMap, path::PathBuf};

	#[derive(Debug, Parser)]
	#[clap(about = "Generates forwarding logic for `typed-glam`")]
	struct Cli {
		#[clap(subcommand)]
		command: Commands,
	}

	#[derive(Debug, Subcommand)]
	enum Commands {
		EchoOne { name: String },
		Rebuild { path: PathBuf },
	}

	struct GeneratedFile {
		file_name: &'static str,
		gen: Box<dyn Fn() -> rust::Tokens>,
	}

	let files = HashMap::from([
		// i32
		(
			"ivec2",
			GeneratedFile {
				file_name: "ivec2.generated.rs",
				gen: Box::new(|| {
					vec_derive::derive_for_vec(
						&rust::import("glam::i32", "IVec2"),
						&rust::import("glam::bool", "BVec2"),
						vec_derive::CompType::I32,
						2,
					)
				}),
			},
		),
		(
			"ivec3",
			GeneratedFile {
				file_name: "ivec3.generated.rs",
				gen: Box::new(|| {
					vec_derive::derive_for_vec(
						&rust::import("glam::i32", "IVec3"),
						&rust::import("glam::bool", "BVec3"),
						vec_derive::CompType::I32,
						3,
					)
				}),
			},
		),
		(
			"ivec4",
			GeneratedFile {
				file_name: "ivec4.generated.rs",
				gen: Box::new(|| {
					vec_derive::derive_for_vec(
						&rust::import("glam::i32", "IVec4"),
						&rust::import("glam::bool", "BVec4"),
						vec_derive::CompType::I32,
						4,
					)
				}),
			},
		),
		// u32
		(
			"uvec2",
			GeneratedFile {
				file_name: "uvec2.generated.rs",
				gen: Box::new(|| {
					vec_derive::derive_for_vec(
						&rust::import("glam::u32", "UVec2"),
						&rust::import("glam::bool", "BVec2"),
						vec_derive::CompType::U32,
						2,
					)
				}),
			},
		),
		(
			"uvec3",
			GeneratedFile {
				file_name: "uvec3.generated.rs",
				gen: Box::new(|| {
					vec_derive::derive_for_vec(
						&rust::import("glam::u32", "UVec3"),
						&rust::import("glam::bool", "BVec3"),
						vec_derive::CompType::U32,
						3,
					)
				}),
			},
		),
		(
			"uvec4",
			GeneratedFile {
				file_name: "uvec4.generated.rs",
				gen: Box::new(|| {
					vec_derive::derive_for_vec(
						&rust::import("glam::u32", "UVec4"),
						&rust::import("glam::bool", "BVec4"),
						vec_derive::CompType::U32,
						4,
					)
				}),
			},
		),
		// f32
		(
			"fvec2",
			GeneratedFile {
				file_name: "fvec2.generated.rs",
				gen: Box::new(|| {
					vec_derive::derive_for_vec(
						&rust::import("glam::f32", "Vec2"),
						&rust::import("glam::bool", "BVec2"),
						vec_derive::CompType::F32,
						2,
					)
				}),
			},
		),
		(
			"fvec3",
			GeneratedFile {
				file_name: "fvec3.generated.rs",
				gen: Box::new(|| {
					vec_derive::derive_for_vec(
						&rust::import("glam::f32", "Vec3"),
						&rust::import("glam::bool", "BVec3"),
						vec_derive::CompType::F32,
						3,
					)
				}),
			},
		),
		(
			"fvec4",
			GeneratedFile {
				file_name: "fvec4.generated.rs",
				gen: Box::new(|| {
					vec_derive::derive_for_vec(
						&rust::import("glam::f32", "Vec4"),
						&rust::import("glam::bool", "BVec4A"),
						vec_derive::CompType::F32,
						4,
					)
				}),
			},
		),
		// f64
		(
			"dvec2",
			GeneratedFile {
				file_name: "dvec2.generated.rs",
				gen: Box::new(|| {
					vec_derive::derive_for_vec(
						&rust::import("glam::f64", "DVec2"),
						&rust::import("glam::bool", "BVec2"),
						vec_derive::CompType::F64,
						2,
					)
				}),
			},
		),
		(
			"dvec3",
			GeneratedFile {
				file_name: "dvec3.generated.rs",
				gen: Box::new(|| {
					vec_derive::derive_for_vec(
						&rust::import("glam::f64", "DVec3"),
						&rust::import("glam::bool", "BVec3"),
						vec_derive::CompType::F64,
						3,
					)
				}),
			},
		),
		(
			"dvec4",
			GeneratedFile {
				file_name: "dvec4.generated.rs",
				gen: Box::new(|| {
					vec_derive::derive_for_vec(
						&rust::import("glam::f64", "DVec4"),
						&rust::import("glam::bool", "BVec4"),
						vec_derive::CompType::F64,
						4,
					)
				}),
			},
		),
	]);

	let cli: Cli = Cli::parse();

	match cli.command {
		Commands::EchoOne { name } => {
			let command = files
				.get(name.as_str())
				.with_context(|| format!("failed to get generated file with ID {name:?}"))?;

			let output = (command.gen)()
				.to_file_string()
				.context("failed to generate file")?;

			println!("{output}");

			// if let Ok(mut cx) = clipboard::ClipboardContext::new() {
			// 	cx.set_contents(output).unwrap();
			// 	eprintln!("The above file has been copied to your clipboard.");
			// } else {
			// 	eprintln!("Failed to get clipboard context.");
			// }
		}
		Commands::Rebuild { mut path } => {
			// Ensure that users have the `_generated.md` marker file
			{
				path.push("_generated.md");
				if !path
					.try_exists()
					.context("failed to check if `_generated.md` exists")?
				{
					bail!(
					"File at {path:?} does not exist. This placeholder file is required to ensure \
					 that users don't accidentally run codegen in an undesired directory."
				);
				}
				path.pop();
			}

			// Remove all existing `.generated.rs` files
			{
				const DEL_ERR: &str =
					"IO operation failed while removing existing `.generated.rs` files.";

				for entry in path.read_dir().context(DEL_ERR)? {
					let entry = entry.context(DEL_ERR)?;
					let entry_path = entry.path();

					let is_file = entry.file_type().context(DEL_ERR)?.is_file();
					let has_proper_ext = entry.path().file_name().map_or(false, |name| {
						name.to_string_lossy().ends_with(".generated.rs")
					});

					if is_file && has_proper_ext {
						eprintln!("Removing {entry_path:?}");
						std::fs::remove_file(entry_path).context(DEL_ERR)?;
					}
				}
			}

			// Create new `.generated.rs` files
			for (id, descriptor) in files {
				path.push(descriptor.file_name);

				eprintln!("Generating file {id:?} to be put in {path:?}");
				let contents = (descriptor.gen)()
					.to_file_string()
					.context("failed to generate file")?;

				std::fs::write(&path, contents).context("failed to write file")?;
				path.pop();
			}

			eprintln!("Finished");
		}
	}

	Ok(())
}
