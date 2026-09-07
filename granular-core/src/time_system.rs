use crate::utils::*;
use web_time::{Duration, Instant};

pub struct TimeSystem {
    _ctx: GeeseContextHandle<Self>,
    engine_start: Instant,
    last_frame: Instant,
}
impl TimeSystem {
    pub(super) fn mark_frame(&mut self) {
        self.last_frame = Instant::now();
    }

    pub fn delta_time(&self) -> Duration {
        self.last_frame.elapsed()
    }

    pub fn time_since_start(&self) -> Duration {
        self.engine_start.elapsed()
    }
}
impl GeeseSystem for TimeSystem {
    fn new(ctx: GeeseContextHandle<Self>) -> Self {
        Self {
            _ctx: ctx,
            engine_start: Instant::now(),
            last_frame: Instant::now(),
        }
    }
}
