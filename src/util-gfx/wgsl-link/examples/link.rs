use std::path::Path;

use wgsl_link::driver::session::{Session, Wgsl};

fn main() {
    let mut sess = Session::new(Wgsl::default());
    eprintln!(
        "{}",
        sess.link(Path::new("src/util-gfx/wgsl-link/examples/entry.wgsl"))
    );
}
