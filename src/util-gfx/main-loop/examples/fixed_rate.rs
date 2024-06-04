use std::{
    num::NonZeroU32,
    thread,
    time::{Duration, Instant},
};

use main_loop::FixedRate;

fn main() {
    let mut fr = FixedRate::new(10.);

    loop {
        let now = Instant::now();
        let ticks = fr.tick(now);

        for _ in 0..ticks.output.map_or(0, NonZeroU32::get) {
            eprintln!("Tick!");
            thread::sleep(Duration::from_millis(fastrand::u64(0..200)))
        }

        std::thread::sleep(ticks.next_tick.saturating_duration_since(Instant::now()));
    }
}
