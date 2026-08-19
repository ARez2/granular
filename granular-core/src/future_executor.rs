use std::{
    future::Future,
    task::{Context, Poll, Waker},
};

use crate::utils::*;

pub mod events {
    // this gets raised by FutureExecutor when f finishes
    pub struct FutureReady<T>(pub T);
}

/// A future that takes a Context (needed for polling), the FutureExecutor (needed to use ctx.raise_event) and returns true if the future is completed
type Task = Box<dyn FnMut(&mut Context<'_>, &mut FutureExecutor) -> bool>;

pub struct FutureExecutor {
    ctx: GeeseContextHandle<Self>,
    tasks: Vec<Task>,
}
impl FutureExecutor {
    /// Spawns the `future` (which shouldn't return anything) and then polls for it to complete.
    pub fn spawn_oneshot(&mut self, future: impl Future<Output = ()> + 'static) {
        let mut future = Box::pin(future);

        self.tasks
            .push(Box::new(move |cx, _| future.as_mut().poll(cx).is_ready()));
    }

    /// Spawns the `future` and then polls for it to complete, raising a `events::FutureReady` upon completion
    pub fn spawn<T>(&mut self, future: impl Future<Output = T> + Send + 'static)
    where
        T: Send + 'static + Sync,
    {
        let mut future = Box::pin(future);

        self.tasks.push(Box::new(move |cx, executor| {
            match future.as_mut().poll(cx) {
                Poll::Pending => false,
                Poll::Ready(value) => {
                    executor.ctx.raise_event(events::FutureReady(value));
                    true
                }
            }
        }));
    }

    /// Event handler which polls all running tasks each time to run them to completion
    fn poll_futures(&mut self, _event: &crate::events::timing::Tick<1>) {
        let waker = Waker::noop();
        let mut cx = Context::from_waker(waker);

        let mut tasks = std::mem::take(&mut self.tasks);
        for mut task in tasks.drain(..) {
            let done = (task)(&mut cx, self);

            if !done {
                self.tasks.push(task);
            }
        }
    }
}
impl GeeseSystem for FutureExecutor {
    const EVENT_HANDLERS: EventHandlers<Self> = event_handlers().with(Self::poll_futures);

    fn new(ctx: GeeseContextHandle<Self>) -> Self {
        Self { ctx, tasks: vec![] }
    }
}
