use geese::*;
use granular_core::{GranularEngine, events, EventLoopSystem};
use log::info;
use winit::event_loop::EventLoop;

fn main() {
    env_logger::init();
    ctx.flush().with(geese::notify::add_system::<EventLoopSystem>());
    
    let mut ctx = GeeseContext::default();
    ctx.flush().with(geese::notify::add_system::<Game>());

    let window_size = winit::dpi::LogicalSize::new(640, 480);
    {
        let mut engine = ctx.get_mut::<GranularEngine>();
        pollster::block_on(
            engine.create_window(
                &event_loop, "Game Window", window_size
            )
        );
    }


    event_loop.run(move |event, target| {
        ctx.flush().with(event);

        let mut engine = ctx.get_mut::<GranularEngine>();
        engine.use_window_target(target);
        // sends out an event which the game will handle
        engine.update();

    }).unwrap();
}


struct Game {
    ctx: GeeseContextHandle<Self>
}
impl Game {
    fn on_update(&mut self, event: &events::NewFrame) {
        info!("Update game");
    }
}
impl GeeseSystem for Game {
    const DEPENDENCIES: Dependencies = dependencies()
        .with::<GranularEngine>();
    const EVENT_HANDLERS: EventHandlers<Self> = event_handlers()
        .with(Self::on_update);

    
    fn new(ctx: GeeseContextHandle<Self>) -> Self {
        Self {
            ctx
        }
    }
}