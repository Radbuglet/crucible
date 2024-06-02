use std::{cell::Cell, hint::black_box, rc::Rc, time::Instant};

fn main() {
    let start = Instant::now();

    let counter = Rc::new(Cell::new(0u64));

    for _ in 0..1_000_000 {
        black_box(&counter).set(black_box(&counter).get() + 1);
    }

    dbg!(start.elapsed());
}
