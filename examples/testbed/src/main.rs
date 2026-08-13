use glam::IVec2;
use granular::prelude::*;
use palette::{Srgba, WithAlpha};
use regex::Regex;
use time::macros::format_description;
use tracing_subscriber::{
    fmt::{self},
    layer::SubscriberExt,
    Layer,
};
use winit::keyboard::{KeyCode, ModifiersState};

mod utils;

const DEFAULT_LOG_FILTER: &str = "wgpu=error,granular=debug,testbed=trace";

fn main() {
    std::env::set_var("RUST_BACKTRACE", "1");
    //std::env::set_var("RUST_LOG", "testbed=trace");

    let path_regex = std::sync::Arc::new(Regex::new(r"(\S*/)$").unwrap());
    let subscriber = tracing_subscriber::registry();
    let filter_layer = tracing_subscriber::EnvFilter::new(DEFAULT_LOG_FILTER);
    let subscriber = subscriber.with(filter_layer);
    let timer = fmt::time::LocalTime::new(format_description!(
        "[hour]:[minute]:[second].[subsecond digits:3]"
    ));
    let show_spans = false;
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_timer(timer)
        .with_file(true)
        .with_line_number(true)
        .with_target(false)
        .with_writer({
            let path_regex = std::sync::Arc::clone(&path_regex);

            move || {
                utils::CleaningWriter::new(std::io::stdout(), std::sync::Arc::clone(&path_regex))
            }
        })
        .with_filter(tracing_subscriber::filter::filter_fn(move |meta| {
            !meta.is_span() || show_spans
        }));

    let subscriber = subscriber.with(fmt_layer);

    #[cfg(feature = "trace")]
    let subscriber = subscriber.with(tracing_tracy::TracyLayer::default());
    #[cfg(target_arch = "wasm32")]
    let subscriber = subscriber.with(tracing_wasm::WASMLayer::new(
        tracing_wasm::WASMLayerConfig::default(),
    ));
    tracing::subscriber::set_global_default(subscriber).unwrap();

    let mut engine = GranularEngine::<Game>::new();
    engine.run();
}

#[derive(Debug)]
struct Game {
    ctx: GeeseContextHandle<Self>,

    texture: AssetHandle<TextureAsset>,
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
        let mut renderer = self.ctx.get_mut::<BatchRenderer>();
        renderer.draw_quad(
            &graphics::Quad {
                center: IVec2::new(500, 300),
                size: IVec2::new(200, 200),
                color: Srgba::from_format(palette::named::WHITE.with_alpha(1.0)),
                texture: Some(self.texture.clone()),
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
        let texture = asset_sys.load::<TextureAsset>("assets/cat2.jpg", true);
        drop(asset_sys);

        Self { ctx, texture }
    }
}
