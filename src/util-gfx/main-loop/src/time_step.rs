use std::{
    num::NonZeroU32,
    time::{Duration, Instant},
};

// === Common === //

pub enum TickResult<T> {
    Tick(T),
    Sleep(Instant),
}

// === LimitedRate === //

pub type LimitedRateTickResult = TickResult<()>;

#[derive(Debug)]
pub struct LimitedRate {
    rate: f64,
    min_delta: Duration,
    last_delta_as_duration: Duration,
    last_delta_mirror: f64,
    last_tick: Option<Instant>,
}

impl LimitedRate {
    pub fn new(rate: f64) -> Self {
        let min_delta = 1. / rate;
        let min_delta_as_duration = Duration::from_secs_f64(min_delta);

        Self {
            rate,
            min_delta: min_delta_as_duration,
            last_delta_as_duration: min_delta_as_duration,
            last_delta_mirror: min_delta,
            last_tick: None,
        }
    }

    pub fn rate(&self) -> f64 {
        self.rate
    }

    pub fn min_delta(&self) -> Duration {
        self.min_delta
    }

    pub fn last_delta(&self) -> f64 {
        self.last_delta_mirror
    }

    pub fn last_delta_as_duration(&self) -> Duration {
        self.last_delta_as_duration
    }

    pub fn tick(&mut self, now: Instant) -> LimitedRateTickResult {
        // If this is our first tick, run it immediately.
        let Some(last_tick) = self.last_tick.as_mut() else {
            self.last_tick = Some(now);
            return TickResult::Tick(());
        };

        // See if we can run yet.
        let delta = now.duration_since(*last_tick);

        if delta > self.min_delta {
            *last_tick = now;
            self.last_delta_as_duration = delta;
            self.last_delta_mirror = delta.as_secs_f64();
            LimitedRateTickResult::Tick(())
        } else {
            LimitedRateTickResult::Sleep(now + (self.min_delta - delta))
        }
    }
}

// === FixedRate === //

pub type FixedRateTickResult = TickResult<NonZeroU32>;

// This mechanism is inspired, in large part, by https://gafferongames.com/post/fix_your_timestep/
#[derive(Debug)]
pub struct FixedRate {
    /// The rate in ticks-per-second that this timer will run at.
    rate: f64,

    /// The delta of each of these ticks in seconds.
    fixed_delta: f64,

    /// The delta of each of these ticks as a duration object.
    fixed_delta_as_duration: Duration,

    /// The time of the last tick in the *tick-rate time space*. This is essentially world time if it
    /// started at program startup.
    world_time: Option<Instant>,
}

impl FixedRate {
    pub fn new(rate: f64) -> Self {
        let fixed_delta = 1. / rate;
        Self {
            rate,
            fixed_delta,
            fixed_delta_as_duration: Duration::from_secs_f64(fixed_delta),
            world_time: None,
        }
    }

    pub fn rate(&self) -> f64 {
        self.rate
    }

    pub fn fixed_delta(&self) -> f64 {
        self.fixed_delta
    }

    pub fn fixed_delta_as_duration(&self) -> Duration {
        self.fixed_delta_as_duration
    }

    pub fn next_tick(&self, now: Instant) -> Instant {
        self.world_time.unwrap_or(now) + self.fixed_delta_as_duration
    }

    pub fn blend_factor(&self, now: Instant) -> f64 {
        let Some(prev_tick) = self.world_time else {
            return 1.;
        };

        (now.duration_since(prev_tick).as_secs_f64() / self.fixed_delta).min(1.)
    }

    pub fn tick(&mut self, now: Instant) -> FixedRateTickResult {
        // If this is our first tick, run it immediately.
        let Some(last_tick) = self.world_time.as_mut() else {
            self.world_time = Some(now);
            return TickResult::Tick(NonZeroU32::new(1).unwrap());
        };

        // The `last_tick` will always be in the past compared to us since a) instants are monotonic
        // and b) we only run complete ticks.
        let since_last_tick = now.duration_since(*last_tick);

        // If we have ticks to run, we should run them. We assume that the consumer is capable of
        // running every single tick but the user is, of course, free to cap this number if they're
        // lagging too far behind.
        let ticks_to_run = (since_last_tick.as_secs_f64() / self.fixed_delta) as u32;

        if let Some(ticks_to_run) = NonZeroU32::new(ticks_to_run) {
            *last_tick += self.fixed_delta_as_duration * ticks_to_run.get();

            TickResult::Tick(ticks_to_run)
        } else {
            TickResult::Sleep(self.next_tick(now))
        }
    }
}
