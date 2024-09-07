use std::{
    env::{current_dir, var},
    fs::{self, create_dir_all},
    path::PathBuf,
    process::exit,
};

use dsl_utils::diagnostic::emit_pretty_diagnostics;

use crate::driver::session::Session;

use super::session::Language;

pub fn run_build_script<'a>(
    lang: impl Language,
    entry_paths: impl IntoIterator<Item = (&'a str, &'a str)>,
) {
    let mut sess = Session::new(lang);
    let mut had_error = false;

    let input_base = current_dir().unwrap();
    let output_base = PathBuf::from(var("OUT_DIR").unwrap());
    eprintln!("cargo::rerun-if-env-changed=OUT_DIR");

    for (input, output) in entry_paths {
        let input = input_base.join(input);
        let output = output_base.join(output);

        create_dir_all(&output).unwrap();

        eprintln!("cargo::rerun-if-changed={}", input.to_str().unwrap());

        for file in input.read_dir().unwrap() {
            let file = file.unwrap();
            let input = file.path();
            let output = output.join(file.file_name());

            if input.extension().and_then(|v| v.to_str()) != Some("wgsl") {
                continue;
            }

            match sess.parse(&input) {
                Ok(module) => {
                    fs::write(output, sess.build([module])).unwrap();
                }
                Err(diag) => {
                    emit_pretty_diagnostics(
                        &mut termcolor::StandardStream::stderr(termcolor::ColorChoice::Auto),
                        sess.span_mgr(),
                        &diag,
                    )
                    .unwrap();
                    had_error = true;
                }
            }
        }
    }

    if had_error {
        exit(1);
    }
}
