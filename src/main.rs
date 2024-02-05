use geese::*;
use granular_core::{GranularEngine, events};
use log::{info, trace};
use std::io::Write;

fn main() {
    std::env::set_var("RUST_BACKTRACE", "1");
    std::env::set_var("RUST_LOG", "granular=trace");
    env_logger::builder()
        .format(|buf, record| {
            let ts = buf.timestamp_micros();
            let ts = ts.to_string();
            let timestamp = &ts[11..ts.len()-1];
            let level = buf.default_styled_level(record.level());
            let width = 27;
            let mod_path = match record.module_path() {
                Some(path) => format!("{:<width$}", path),
                None => format!("{:width$}", ""),
            };
            writeln!(buf, "[{ts} {lvl} {path}]: {msg}", ts=timestamp, lvl=level, path=buf.style().set_dimmed(true).value(mod_path), msg=record.args())
        })
        .init();

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
        //trace!("Update game");
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