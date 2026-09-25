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
    let engine = GranularEngine::<Game>::new(uvec2(640, 480));
    engine.run();
}

#[derive(Debug)]
struct Game {
    ctx: GeeseContextHandle<Self>,

    texture_handle: AssetHandle<TextureBundle>,
    texture2_handle: AssetHandle<TextureBundle>,
}
impl Game {
    // Called when everything in the engine is ready
    fn init(&mut self, _event: &events::Initialized) {
        {
            let mut camera = self.ctx.get_mut::<Camera>();
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
        let mut batchrenderer = self.ctx.get_mut::<BatchRenderer>();
        batchrenderer.draw_quad_with_center(
            vec2(0.0, 0.0),
            vec2(250.0, 250.0),
            0.0,
            palette::named::WHITE,
            QuadTex::Texture(self.texture_handle.clone()),
            0,
            DrawSpace::World,
        );
        batchrenderer.draw_quad_with_bottomleft(
            vec2(75.0, 75.0),
            vec2(150.0, 150.0),
            -20.0f32.to_radians(),
            palette::named::WHITE,
            QuadTex::Texture(self.texture2_handle.clone()),
            1,
            DrawSpace::World,
        );
        drop(batchrenderer);
        let time = self.ctx.get::<TimeSystem>().time_since_start();
        let time_secs = time.as_secs_f32();
        let mut debug_draw = self.ctx.get_mut::<DebugDraw>();
        let start = vec2(-200.0, 150.0);
        let end = start + vec2(time_secs.sin(), time_secs.cos()) * 125.0;
        debug_draw.draw_line(start, end, palette::named::LIME, 5.0, 1, DrawSpace::World);

        debug_draw.draw_rect_center(
            vec2(-200.0, 50.0),
            vec2(200.0, 100.0),
            0.0,
            palette::named::RED,
            5.0,
            2,
            DrawSpace::World,
        );
    }
}
impl GeeseSystem for Game {
    // Register our functions to be called from the engine on certain events.
    // The name of those functions is decided by the user
    const EVENT_HANDLERS: EventHandlers<Self> = event_handlers()
        .with(Self::init)
        .with(Self::on_update)
        .with(Self::on_draw);

    // Tell the engine which systems the game needs access to
    const DEPENDENCIES: Dependencies = dependencies()
        .with::<Mut<WindowSystem>>()
        .with::<Mut<InputSystem>>()
        .with::<Mut<Camera>>()
        .with::<Mut<GraphicsSystem>>()
        .with::<Mut<DebugDraw>>()
        .with::<TimeSystem>()
        .with::<Mut<AssetSystem>>()
        .with::<Mut<BatchRenderer>>();

    // Called when the game system gets instantiated (but not everything might be ready yet)
    fn new(mut ctx: GeeseContextHandle<Self>) -> Self {
        info!("Game created");

        // Register arrow keys to use as camera movement
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

        // Load two images
        let (texture_handle, texture2_handle) = {
            let mut asset_sys = ctx.get_mut::<AssetSystem>();
            let texture_handle = asset_sys
                .load(
                    asset_source!("../../assets/cat2.jpg"),
                    TextureBundleLoadSettings {
                        name: String::from("cat"),
                        ..Default::default()
                    },
                )
                .unwrap();
            let texture2_handle = asset_sys
                .load(
                    asset_source!("../../assets/cat3.jpg"),
                    TextureBundleLoadSettings::default(),
                )
                .unwrap();
            (texture_handle, texture2_handle)
        };

        // Configure the window
        {
            let mut win_sys = ctx.get_mut::<WindowSystem>();
            win_sys.set_window_size(winit::dpi::PhysicalSize::new(865, 559));
            win_sys.set_title("Granular engine minimal example");
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
