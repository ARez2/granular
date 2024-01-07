use geese::*;
use winit::event_loop::EventLoop;


pub struct EventLoopSystem {
    event_loop: EventLoop<()>
}
impl EventLoopSystem {
    pub fn get(&self) -> &EventLoop<()> {
        &self.event_loop
    }

    pub fn get_mut(&mut self) -> &mut EventLoop<()> {
        &mut self.event_loop
    }
}
impl GeeseSystem for EventLoopSystem {
    fn new(ctx: GeeseContextHandle<Self>) -> Self {
        Self {
            event_loop: EventLoop::new().unwrap(),
        }
    }
}