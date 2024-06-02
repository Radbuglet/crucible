use std::{
    thread,
    time::{Duration, Instant},
};

use winit_ext::{FixedRate, TickResult};

fn main() {
    let mut fr = FixedRate::new(10.);

    loop {
        let now = Instant::now();
        let ticks = match fr.tick(now) {
            TickResult::Tick(ticks) => ticks,
            TickResult::Sleep(instant) => {
                thread::sleep(instant.duration_since(now));
                continue;
            }
        };

        for _ in 0..(ticks.get().min(2)) {
            eprintln!("Tick!");
            thread::sleep(Duration::from_millis(fastrand::u64(0..200)))
        }
    }
}
