use std::{cell::Cell, hint::black_box, time::Instant};

thread_local! {
    static TLS: Cell<u64> = const { Cell::new(0) };
}

fn main() {
    let start = Instant::now();

    for _ in 0..1_000_000 {
        TLS.with(|tls| {
            tls.set(tls.get() + 1);
        });
        black_box(());
    }

    dbg!(start.elapsed());
}
