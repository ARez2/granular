use web_time::{Duration, Instant};

/// A small helper to help time fixed updates
pub struct FixedStepper {
    step: Duration,
    accumulator: Duration,
    max_steps: u32,
    last_step: Instant,
}

impl FixedStepper {
    /// `max_steps` is how many steps are returned at maximum by `advance(...)`
    pub fn new(hz: u32, max_steps: u32) -> Self {
        assert!(max_steps > 0);
        Self {
            step: Duration::from_secs_f64(1.0 / f64::from(hz)),
            accumulator: Duration::ZERO,
            max_steps,
            last_step: Instant::now(),
        }
    }

    pub fn set_hz(&mut self, hz: u32) {
        let step = Duration::from_secs_f64(1.0 / f64::from(hz));
        if step != self.step {
            self.step = step;
            self.reset();
        }
    }

    pub fn hz(&self) -> u32 {
        (1.0 / self.step.as_secs_f64()).round() as u32
    }

    pub fn reset(&mut self) {
        self.accumulator = Duration::ZERO;
        self.last_step = Instant::now();
    }

    pub fn delta(&self) -> f32 {
        self.step.as_secs_f32()
    }

    /// Returns the number of steps to run.
    /// Excess whole steps are discarded if the catch-up limit is reached.
    pub fn advance(&mut self) -> u32 {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_step);
        self.last_step = now;

        let total = self.accumulator.as_nanos() + elapsed.as_nanos();
        let step = self.step.as_nanos();

        let steps = (total / step).min(self.max_steps as u128) as u32;
        self.accumulator = Duration::from_nanos((total % step) as u64);
        steps
    }
}
