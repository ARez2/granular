use geese::*;
use granular_core::{GranularEngine, events};
use log::*;
use std::io::Write;
use regex::Regex;

fn main() {
    std::env::set_var("RUST_BACKTRACE", "1");
    std::env::set_var("RUST_LOG", "granular=trace");

    // Matches a full path until (excluding) "granular"
    let path_regex = Regex::new(r" \b(.*)\bgranular\b").unwrap();
    env_logger::builder()
        .format(move |buf, record| {
            let ts = buf.timestamp_millis();
            let ts = ts.to_string();
            let timestamp = &ts[11..ts.len()-1];
            let level = buf.default_styled_level(record.level());
            let width = 27;
            let mod_path = match record.module_path() {
                Some(path) => format!("{:<width$}", path),
                None => format!("{:width$}", ""),
            };
            
            // Remove personal stuff from full path
            let mut msg_clean = record.args().to_string();
            if let Some(re_match) = path_regex.captures(&msg_clean) {
                if let Some(pre_path) = re_match.get(1) {
                    msg_clean.replace_range(pre_path.start()..pre_path.end(), "");
                }
            };
            writeln!(buf, "[{ts} {lvl} {path}]: {msg}", ts=timestamp, lvl=level, path=buf.style().set_dimmed(true).value(mod_path), msg=msg_clean)
        })
        .init();

    let window_size = Some(winit::dpi::PhysicalSize::new(640, 480));

    let mut engine = GranularEngine::new();
    engine.get_ctx().flush().with(geese::notify::add_system::<Game>());
    engine.create_window("Granular", window_size);
    engine.run();
}


struct Game {
    _ctx: GeeseContextHandle<Self>
}
impl Game {
    fn on_update(&mut self, _: &events::timing::FixedTick) {
        info!("Fixed game update");
    }
}
impl GeeseSystem for Game {
    const EVENT_HANDLERS: EventHandlers<Self> = event_handlers()
        .with(Self::on_update);
    
    fn new(ctx: GeeseContextHandle<Self>) -> Self {
        info!("Game created");
        Self {
            _ctx: ctx
        }
    }
}