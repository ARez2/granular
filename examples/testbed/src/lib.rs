use fern::colors::{Color, ColoredLevelConfig};
use glam::IVec2;
use granular::prelude::{graphics::GraphicsSystem, *};
use palette::{Srgba, WithAlpha};
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

/// Settings you might want to set when loading a texture. Not complete
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct TextureSettings {
    pub size: granular::wgpu::Extent3d,
    pub format: granular::wgpu::TextureFormat,
    pub filtering: granular::wgpu::FilterMode,
}
impl Default for TextureSettings {
    fn default() -> Self {
        Self {
            size: granular::wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            format: granular::wgpu::TextureFormat::Rgba8UnormSrgb,
            filtering: granular::wgpu::FilterMode::Nearest,
        }
    }
}

#[derive(Debug)]
struct Game {
    ctx: GeeseContextHandle<Self>,

    texture_handle: AssetHandle<TextureBundle>,
    texture2_handle: AssetHandle<TextureBundle>,
}
impl Game {
    fn init(&mut self, _event: &events::Initialized) {
        let win_sys = self.ctx.get::<WindowSystem>();
        let window = win_sys.window_handle();
        window.set_visible(true);
        let s = window.request_inner_size(winit::dpi::PhysicalSize::new(640, 480));
        debug!("Game set_min_inner_size {:?}", s);
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
        let mut renderer = self.ctx.get_mut::<BatchRenderer>();
        renderer.draw_quad(
            &graphics::Quad {
                center: IVec2::new(50, 50),
                size: IVec2::new(200, 200),
                color: Srgba::from_format(palette::named::WHITE.with_alpha(1.0)),
                texture: Some(self.texture_handle.clone()),
            },
            -2,
        );
        renderer.draw_quad(
            &graphics::Quad {
                center: IVec2::new(200, 200),
                size: IVec2::new(100, 100),
                color: Srgba::from_format(palette::named::WHITE.with_alpha(1.0)),
                texture: None,
            },
            1,
        );
        renderer.draw_quad(
            &graphics::Quad {
                center: IVec2::new(40, 300),
                size: IVec2::new(150, 150),
                color: Srgba::from_format(palette::named::WHITE.with_alpha(1.0)),
                texture: Some(self.texture2_handle.clone()),
            },
            0,
        );
    }

    const EVENT_HANDLERS_SHARED: EventHandlers<Self> = event_handlers()
        .with(Self::init)
        .with(Self::on_update)
        .with(Self::on_draw);
}
impl GeeseSystem for Game {
    const EVENT_HANDLERS: EventHandlers<Self> = Self::EVENT_HANDLERS_SHARED;

    const DEPENDENCIES: Dependencies = dependencies()
        .with::<WindowSystem>()
        .with::<GraphicsSystem>()
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

        let (device, queue) = {
            let graphics_sys = ctx.get::<GraphicsSystem>();
            (graphics_sys.device().clone(), graphics_sys.queue().clone())
        };
        let load_image = move |bytes: Vec<u8>, settings: TextureSettings| {
            Ok(TextureBundle::new(
                &device,
                &queue,
                "TextureAsset",
                granular::wgpu::TextureDescriptor {
                    label: Some("TextureAsset Desc"),
                    size: settings.size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: granular::wgpu::TextureDimension::D2,
                    format: settings.format,
                    usage: granular::wgpu::TextureUsages::TEXTURE_BINDING
                        | granular::wgpu::TextureUsages::COPY_DST
                        | granular::wgpu::TextureUsages::COPY_SRC,
                    view_formats: &[],
                },
                &granular::wgpu::TextureViewDescriptor::default(),
                &granular::wgpu::SamplerDescriptor {
                    address_mode_u: granular::wgpu::AddressMode::ClampToEdge,
                    address_mode_v: granular::wgpu::AddressMode::ClampToEdge,
                    address_mode_w: granular::wgpu::AddressMode::ClampToEdge,
                    mag_filter: settings.filtering,
                    min_filter: granular::wgpu::FilterMode::Nearest,
                    mipmap_filter: match settings.filtering {
                        granular::wgpu::FilterMode::Linear => {
                            granular::wgpu::MipmapFilterMode::Linear
                        }
                        granular::wgpu::FilterMode::Nearest => {
                            granular::wgpu::MipmapFilterMode::Nearest
                        }
                    },
                    ..Default::default()
                },
                &bytes,
                granular::wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(
                        crate::graphics::bytes_per_pixel(settings.format).unwrap_or(4)
                            * settings.size.width,
                    ),
                    rows_per_image: Some(settings.size.height),
                },
            ))
        };

        let (texture_handle, texture2_handle) = {
            let mut asset_sys = ctx.get_mut::<AssetSystem>();
            let load1 = load_image.clone();
            let texture_handle = asset_sys
                .load(asset_source!("../../assets/cat.jpg"), move |bytes| {
                    load1(bytes, TextureSettings::default())
                })
                .unwrap();
            let texture2_handle = asset_sys
                .load(asset_source!("../../assets/cat2.jpg"), move |bytes| {
                    load_image(bytes, TextureSettings::default())
                })
                .unwrap();
            (texture_handle, texture2_handle)
        };

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
