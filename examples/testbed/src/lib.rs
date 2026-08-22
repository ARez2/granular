use fern::colors::{Color, ColoredLevelConfig};
use glam::IVec2;
use granular::prelude::{graphics::TextureHandle, *};
use palette::{Srgba, WithAlpha};
use web_time::SystemTime;
use winit::keyboard::{KeyCode, ModifiersState};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn run_wasm() -> Result<(), wasm_bindgen::JsValue> {
    console_error_panic_hook::set_once();

    run();
    Ok(())
}

pub fn run() {
    set_up_logging();
    let engine = GranularEngine::<Game>::new();
    engine.run();
}

#[derive(Debug)]
struct Game {
    ctx: GeeseContextHandle<Self>,

    asset: AssetHandle<TextureAsset>,
    texture: Option<TextureHandle>,
}
impl Game {
    fn init(&mut self, _event: &events::Initialized) {
        let win_sys = self.ctx.get::<WindowSystem>();
        let window = win_sys.window_handle();
        window.set_visible(true);
        window.set_min_inner_size(Some(winit::dpi::PhysicalSize::new(640, 480)));
        window.set_title("Granular engine testbed");
    }

    fn on_update(&mut self, _: &events::timing::FixedTick<16>) {
        let input = self.ctx.get::<InputSystem>();
        let vector = input.get_input_vector("cam_left", "cam_right", "cam_up", "cam_down");
        drop(input);
        let mut camera = self.ctx.get_mut::<Camera>();
        camera.translate(vector * 10);
        let _pos = camera.position();
        drop(camera);
    }

    fn on_draw(&mut self, _: &events::Draw) {
        let mut asset_sys = self.ctx.get_mut::<AssetSystem>();
        if asset_sys.status(&self.asset) == AssetStatus::Ready {
            self.texture = Some(asset_sys.get_mut(&self.asset).unwrap().handle().clone());
        }
        drop(asset_sys);
        let mut renderer = self.ctx.get_mut::<BatchRenderer>();
        renderer.draw_quad(
            &graphics::Quad {
                center: IVec2::new(500, 300),
                size: IVec2::new(200, 200),
                color: Srgba::from_format(palette::named::WHITE.with_alpha(1.0)),
                texture: self.texture.clone(),
            },
            -1,
        );
        renderer.draw_quad(
            &graphics::Quad {
                center: IVec2::new(500, 300),
                size: IVec2::new(100, 100),
                color: Srgba::from_format(palette::named::WHITE.with_alpha(1.0)),
                texture: None,
            },
            1,
        );
    }
}
impl GeeseSystem for Game {
    const EVENT_HANDLERS: EventHandlers<Self> = event_handlers()
        .with(Self::init)
        .with(Self::on_update)
        .with(Self::on_draw);

    const DEPENDENCIES: Dependencies = dependencies()
        .with::<WindowSystem>()
        .with::<Mut<InputSystem>>()
        .with::<Mut<Camera>>()
        .with::<Mut<AssetSystem>>()
        .with::<Mut<BatchRenderer>>();

    fn new(mut ctx: GeeseContextHandle<Self>) -> Self {
        info!("Game created");

        let mut input = ctx.get_mut::<InputSystem>();
        input.add_action(
            "cam_left",
            InputActionTrigger::new_key(KeyCode::ArrowLeft, ModifiersState::empty()),
        );
        input.add_action(
            "cam_right",
            InputActionTrigger::new_key(KeyCode::ArrowRight, ModifiersState::empty()),
        );
        input.add_action(
            "cam_up",
            InputActionTrigger::new_key(KeyCode::ArrowUp, ModifiersState::empty()),
        );
        input.add_action(
            "cam_down",
            InputActionTrigger::new_key(KeyCode::ArrowDown, ModifiersState::empty()),
        );
        drop(input);

        let mut asset_sys = ctx.get_mut::<AssetSystem>();
        let asset = asset_sys.load::<TextureAsset>(
            "assets/cat2.jpg",
            true,
            TextureAssetImportSettings {
                size: Extent3d {
                    width: 563,
                    height: 565,
                    depth_or_array_layers: 1,
                },
                ..Default::default()
            },
        );

        drop(asset_sys);

        Self {
            ctx,
            texture: None,
            asset,
        }
    }
}

fn set_up_logging() {
    // configure colors for the whole line
    let colors_line = ColoredLevelConfig::new()
        .error(Color::Red)
        .warn(Color::Yellow)
        // we actually don't need to specify the color for debug and info, they are white by default
        .info(Color::White)
        .debug(Color::Blue)
        // depending on the terminals color scheme, this is the same as the background color
        .trace(Color::BrightBlack);

    // configure colors for the name of the level.
    // since almost all of them are the same as the color for the whole line, we
    // just clone `colors_line` and overwrite our changes
    let colors_level = colors_line.info(Color::Green);
    // here we set up our fern Dispatch
    #[cfg(target_arch = "wasm32")]
    let mut disp = fern::Dispatch::new().format(move |out, message, record| {
        out.finish(format_args!(
            "{color_line}[{level} {target} {color_line}] {message}\x1B[0m",
            color_line = format_args!(
                "\x1B[{}m",
                colors_line.get_color(&record.level()).to_fg_str()
            ),
            target = record.target(),
            level = colors_level.color(record.level()),
            message = message,
        ));
    });

    #[cfg(not(target_arch = "wasm32"))]
    let mut disp = fern::Dispatch::new().format(move |out, message, record| {
        out.finish(format_args!(
            "{color_line}[{date} {level} {target} {color_line}] {message}\x1B[0m",
            color_line = format_args!(
                "\x1B[{}m",
                colors_line.get_color(&record.level()).to_fg_str()
            ),
            date = humantime::format_rfc3339_micros(SystemTime::now()),
            target = record.target(),
            level = colors_level.color(record.level()),
            message = message,
        ));
    });
    disp = disp
        // set the default log level. to filter out verbose log messages from dependencies, set
        // this to Warn and overwrite the log level for your crate.
        .level(log::LevelFilter::Warn)
        // change log levels for individual modules. Note: This looks for the record's target
        // field which defaults to the module path but can be overwritten with the `target`
        // parameter:
        // `info!(target="special_target", "This log message is about special_target");`
        .level_for("wgpu", log::LevelFilter::Error)
        .level_for("granular_core", log::LevelFilter::Trace)
        .level_for("testbed", log::LevelFilter::Trace);
    #[cfg(target_arch = "wasm32")]
    {
        disp = disp.chain(fern::Output::call(console_log::log));
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        // output to stdout
        disp = disp.chain(std::io::stdout())
    }
    disp.apply().unwrap();
}
