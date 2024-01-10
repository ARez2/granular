use geese::*;
use granular_core::{GranularEngine, events, EventLoopSystem};
use log::{info, trace};

fn main() {
    std::env::set_var("RUST_BACKTRACE", "1");
    std::env::set_var("RUST_LOG", "granular,wgpu=debug");
    env_logger::init();

    let window_size = Some(winit::dpi::PhysicalSize::new(640, 480));

    let mut engine = GranularEngine::new();
    engine.get_ctx().flush().with(geese::notify::add_system::<Game>());
    engine.create_window("Granular", window_size);
    engine.run();
}


struct Game {
    ctx: GeeseContextHandle<Self>
}
impl Game {
    fn on_update(&mut self, event: &events::NewFrame) {
        trace!("Update game");
    }
}
impl GeeseSystem for Game {
    const EVENT_HANDLERS: EventHandlers<Self> = event_handlers()
        .with(Self::on_update);

    
    fn new(ctx: GeeseContextHandle<Self>) -> Self {
        info!("Game created");
        Self {
            ctx
        }
    }
}