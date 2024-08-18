use std::path::Path;

use lang_utils::diagnostic::emit_pretty_diagnostics;
use wgsl_link::driver::session::{Session, Wgsl};

fn main() {
    let mut sess = Session::new(Wgsl::default());

    match sess.link(Path::new("src/util-gfx/wgsl-link/examples/entry.wgsl")) {
        Ok(out) => eprintln!("{out}"),
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
