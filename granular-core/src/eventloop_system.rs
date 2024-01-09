use std::sync::Arc;

use geese::*;
use winit::event_loop::EventLoop;


pub struct EventLoopSystem {
    event_loop: Option<EventLoop<()>>
}
impl EventLoopSystem {
    pub fn get(&self) -> &EventLoop<()> {
        if self.event_loop.is_none() {
            panic!("Event loop was already taken!");
        };
        self.event_loop.as_ref().unwrap()
    }

    pub fn get_mut(&mut self) -> &mut EventLoop<()> {
        if self.event_loop.is_none() {
            panic!("Event loop was already taken!");
        };
        self.event_loop.as_mut().unwrap()
    }

    pub fn take(&mut self) -> EventLoop<()> {
        if self.event_loop.is_none() {
            panic!("Event loop was already taken!");
        };
        self.event_loop.take().unwrap()
    }
}
impl GeeseSystem for EventLoopSystem {
    fn new(ctx: GeeseContextHandle<Self>) -> Self {
        Self {
            event_loop: Some(EventLoop::new().unwrap()),
        }
    }
}