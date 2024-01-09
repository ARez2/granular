use geese::*;
use granular_core::{GranularEngine, events, EventLoopSystem};
use log::info;
use winit::platform::run_on_demand::EventLoopExtRunOnDemand;

fn main() {
    env_logger::init();

    let mut ctx = GeeseContext::default();
    //ctx.flush().with(geese::notify::add_system::<EventLoopSystem>());
    ctx.flush().with(geese::notify::add_system::<Game>());

    let window_size = Some(winit::dpi::PhysicalSize::new(640, 480));
    let engine = ctx.get_mut::<GranularEngine>();
    engine.create_window("My game", window_size);
    drop(engine);

    // let mut engine = ctx.get_mut::<GranularEngine>();
    // engine.run(ctx.clone());
    let mut event_loop_sys = ctx.get_mut::<EventLoopSystem>();
    let event_loop = event_loop_sys.take();
    drop(event_loop_sys);
    event_loop.run(move |event, target| {
        ctx.flush().with(event);
        let mut engine = ctx.get_mut::<GranularEngine>();
        engine.use_window_target(target);
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