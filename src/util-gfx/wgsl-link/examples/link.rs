use std::path::Path;

use lang_utils::diagnostic::emit_pretty_diagnostics;
use wgsl_link::driver::session::{Session, Wgsl};

fn main() {
    let mut sess = Session::new(Wgsl::default());

    match sess.parse(Path::new("src/util-gfx/wgsl-link/examples/entry.wgsl")) {
        Ok(module) => {
            eprintln!("{}", sess.build([module]));
        }
        Err(diags) => {
            emit_pretty_diagnostics(
                &mut termcolor::StandardStream::stderr(termcolor::ColorChoice::Always),
                sess.span_mgr(),
                &diags,
            )
            .unwrap();
        }
    }
}
