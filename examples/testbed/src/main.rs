use std::io::Write;

use glam::IVec2;
use granular::prelude::*;
use geese::{dependencies, event_handlers, Dependencies, EventHandlers, EventQueue, GeeseContextHandle, GeeseSystem, Mut};
use palette::{Srgba, WithAlpha};
use regex::Regex;
use log::*;
use winit::keyboard::{KeyCode, ModifiersState};


fn main() {
    std::env::set_var("RUST_BACKTRACE", "1");
    std::env::set_var("RUST_LOG", "granular=debug");

    // Matches a full path until (excluding) "granular"
    let path_regex = Regex::new(r" (.*)\bgranular\b").unwrap();
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
    ctx: GeeseContextHandle<Self>,

    texture: AssetHandle<TextureAsset>
}
impl Game {
    fn on_update(&mut self, _: &events::timing::Tick::<1>) {
        let input = self.ctx.get::<InputSystem>();
        let vector = input.get_input_vector("cam_left", "cam_right", "cam_up", "cam_down");
        drop(input);
        let mut camera = self.ctx.get_mut::<Camera>();
        camera.translate(vector * -2)
    }


    fn on_draw(&mut self, _: &events::Draw) {
        let mut renderer = self.ctx.get_mut::<BatchRenderer>();
        renderer.draw_quad(&graphics::Quad {
            center: IVec2::new(0, 0),
            size: IVec2::new(200, 200),
            color: Srgba::from_format(palette::named::WHITE.with_alpha(1.0)),
            texture: Some(self.texture.clone())
        }, 0);
        renderer.draw_quad(&graphics::Quad {
            center: IVec2::new(500, 0),
            size: IVec2::new(200, 200),
            color: Srgba::from_format(palette::named::RED.with_alpha(1.0)),
            texture: None
        }, 0);
        renderer.draw_quad(&graphics::Quad {
            center: IVec2::new(0, 0),
            size: IVec2::new(100, 100),
            color: Srgba::from_format(palette::named::WHITE.with_alpha(1.0)),
            texture: None
        }, 1);
    }
}
impl GeeseSystem for Game {
    const EVENT_HANDLERS: EventHandlers<Self> = event_handlers()
        .with(Self::on_update)
        .with(Self::on_draw);

    const DEPENDENCIES: Dependencies = dependencies()
        .with::<Mut<InputSystem>>()
        .with::<Mut<Camera>>()
        .with::<Mut<AssetSystem>>()
        .with::<Mut<BatchRenderer>>();
    
    fn new(mut ctx: GeeseContextHandle<Self>) -> Self {
        info!("Game created");
        
        let mut input = ctx.get_mut::<InputSystem>();
        input.add_action("cam_left", InputActionTrigger::new_key(KeyCode::ArrowLeft, ModifiersState::empty()));
        input.add_action("cam_right", InputActionTrigger::new_key(KeyCode::ArrowRight, ModifiersState::empty()));
        input.add_action("cam_up", InputActionTrigger::new_key(KeyCode::ArrowUp, ModifiersState::empty()));
        input.add_action("cam_down", InputActionTrigger::new_key(KeyCode::ArrowDown, ModifiersState::empty()));
        drop(input);

        let mut asset_sys = ctx.get_mut::<AssetSystem>();
        let texture = asset_sys.load::<TextureAsset>("assets/cat2.jpg", true);
        drop(asset_sys);

        Self {
            ctx,
            texture
        }
    }
}