use std::time::{Instant, Duration};

use geese::*;

/// Provides data about tick-based timings.
#[derive(Debug)]
pub struct TickTiming {
    ctx: GeeseContextHandle<Self>,
    last_tick: Instant,
    interval: Duration,
    tick_count: u64
}

impl TickTiming {
    /// Returns the last time that this timer emitted a tick event.
    pub fn last_tick(&self) -> Instant {
        self.last_tick
    }

    /// Provides the next time that this timer will tick.
    pub fn next_tick(&self) -> Instant {
        self.last_tick + self.interval
    }

    /// Sets the interval of this tick timer to the given value.
    pub fn set_interval(&mut self, interval: Duration) {
        self.interval = interval;
    }

    /// Retrieves the number of ticks that have occurred since this
    /// tick timer was added to the context.
    pub fn tick_count(&self) -> u64 {
        self.tick_count
    }

    /// Determines whether it is time for the next tick event,
    /// and if so, emits the event.
    fn handle_next_tick(&mut self, _: &notify::Poll) {
        let current = Instant::now();
        if self.last_tick + self.interval < current {
            self.last_tick += self.interval;
            self.tick_count += 1;
            self.ctx.raise_event(on::Tick);
        }
    }

    /// Changes the interval of this tick timer.
    fn update_interval(&mut self, event: &notify::SetInterval) {
        self.interval = event.0;
    }
}

impl GeeseSystem for TickTiming {
    const EVENT_HANDLERS: EventHandlers<Self> = event_handlers()
        .with(Self::handle_next_tick)
        .with(Self::update_interval);

    fn new(ctx: GeeseContextHandle<Self>) -> Self {
        let interval = Duration::from_secs(1);
        let last_tick = Instant::now();
        let tick_count = 0;
        ctx.raise_event(on::Tick);

        Self { ctx, last_tick, interval, tick_count }
    }
}