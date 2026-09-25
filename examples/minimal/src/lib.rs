use fern::colors::{Color, ColoredLevelConfig};
#[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
use granular::prelude::*;
use winit::keyboard::{KeyCode, ModifiersState};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

/// Wasm entrypoint
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn run_wasm() -> Result<(), wasm_bindgen::JsValue> {
    console_error_panic_hook::set_once();

    run();
    Ok(())
}

/// Real entry point of the program no matter if being run via WASM or native
pub fn run() {
    set_up_logging();
    let engine = GranularEngine::<Game>::new(UVec2::new(640, 480));
    engine.run();
}

#[derive(Debug)]
struct Game {
    ctx: GeeseContextHandle<Self>,

    texture_handle: AssetHandle<TextureBundle>,
    texture2_handle: AssetHandle<TextureBundle>,
}
impl Game {
    fn init(&mut self, _event: &events::Initialized) {
        {
            let mut camera = self.ctx.get_mut::<Camera>();
            camera.set_bottomleft_position(Vec2::ZERO);
            camera.set_motion(CameraMotion::SmoothPixel);
            camera.set_scaling_mode(ScalingMode::KeepAspect);
        }
    }

    fn on_update(&mut self, _: &events::timing::FixedTick<16>) {
        let input = self.ctx.get::<InputSystem>();
        // Gets a 2D vector where the x component is -1 if the cam_left action is pressed, 1 if cam_right is pressed and 0 otherwise. Same for y component
        let vector = input.world_input_direction("cam_left", "cam_right", "cam_up", "cam_down");
        drop(input);
        let mut camera = self.ctx.get_mut::<Camera>();
        // Move the camera using the preconfigured input actions
        camera.translate(vector * 10.0);
    }

    fn on_draw(&mut self, _: &granular::graphics::events::RecordGameRenderingCommands) {
        let mut renderer = self.ctx.get_mut::<BatchRenderer>();
        renderer.draw_quad_with_center(
            Vec2::new(100.0, 250.0),
            Vec2::new(50.0, 50.0),
            f32::to_radians(-45.0),
            palette::named::WHITE,
            QuadTex::Texture(self.texture_handle.clone()),
            -2,
            DrawSpace::World,
        );
        renderer.draw_quad_with_center(
            Vec2::new(50.0, 300.0),
            Vec2::new(50.0, 50.0),
            0.0,
            palette::named::WHITE,
            QuadTex::Texture(self.texture2_handle.clone()),
            0,
            DrawSpace::World,
        );
    }
}
impl GeeseSystem for Game {
    const EVENT_HANDLERS: EventHandlers<Self> = event_handlers()
        .with(Self::init)
        .with(Self::on_update)
        .with(Self::on_draw);

    const DEPENDENCIES: Dependencies = dependencies()
        .with::<Mut<WindowSystem>>()
        .with::<Mut<InputSystem>>()
        .with::<Mut<Camera>>()
        .with::<Mut<GraphicsSystem>>()
        .with::<Mut<AssetSystem>>()
        .with::<Mut<BatchRenderer>>();

    fn new(mut ctx: GeeseContextHandle<Self>) -> Self {
        info!("Game created");

        {
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
        }

        let (texture_handle, texture2_handle) = {
            let mut asset_sys = ctx.get_mut::<AssetSystem>();
            let texture_handle = asset_sys
                .load(
                    asset_source!("../../assets/cat.jpg"),
                    TextureBundleLoadSettings {
                        name: String::from("cat"),
                        ..Default::default()
                    },
                )
                .unwrap();
            let texture2_handle = asset_sys
                .load(
                    asset_source!("../../assets/cat2.jpg"),
                    TextureBundleLoadSettings {
                        name: String::from("cat2"),
                        ..Default::default()
                    },
                )
                .unwrap();
            (texture_handle, texture2_handle)
        };

        {
            let mut win_sys = ctx.get_mut::<WindowSystem>();
            win_sys.set_window_size(winit::dpi::PhysicalSize::new(865, 559));
            win_sys.set_title("Granular engine testbed");
        }

        Self {
            ctx,
            texture_handle,
            texture2_handle,
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
    let pre_date_string_closure = move |record: &log::Record<'_>| {
        format!(
            "{color_line}{bold}[",
            color_line = format_args!(
                "\x1B[{}m",
                colors_line.get_color(&record.level()).to_fg_str()
            ),
            bold = "\x1B[1m",
        )
    };
    let date_string = {
        if cfg!(not(target_arch = "wasm32")) {
            format_time()
        } else {
            String::new()
        }
    };
    let post_date_string_closure =
        move |message: &core::fmt::Arguments<'_>, record: &log::Record<'_>| {
            format!(
                "{level} {bold}{target} {color_line}]{reset} {message}{reset}",
                color_line = format_args!(
                    "\x1B[{}m",
                    colors_line.get_color(&record.level()).to_fg_str()
                ),
                bold = "\x1B[1m",
                reset = "\x1B[0m",
                target = record.target(),
                level = colors_level.color(record.level()),
                message = message,
            )
        };
    // here we set up our fern Dispatch
    let mut disp = fern::Dispatch::new().format(move |out, message, record| {
        out.finish(format_args!(
            "{pre_date_string}{date_string}{post_date_string}",
            pre_date_string = pre_date_string_closure(record),
            post_date_string = post_date_string_closure(message, record)
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
        .level_for("simulation", log::LevelFilter::Trace)
        .level_for("preprocessor", log::LevelFilter::Trace)
        .level_for("granular_core", log::LevelFilter::Trace)
        .level_for(
            "granular_core::graphics::batchrenderer",
            log::LevelFilter::Debug,
        )
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

fn format_time() -> String {
    use std::time::SystemTime;
    use time::{OffsetDateTime, UtcOffset};

    let now = SystemTime::now();
    let timestamp = OffsetDateTime::from(now);
    let offset = UtcOffset::current_local_offset().expect("Could not determine local timezone");
    let local = timestamp.to_offset(offset);

    format!(
        "{:02}:{:02}:{:02}.{:06} ",
        local.hour(),
        local.minute(),
        local.second(),
        local.microsecond(),
    )
}
