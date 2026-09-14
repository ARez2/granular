use fern::colors::{Color, ColoredLevelConfig};
use granular::{
    graphics::{BindGroupBuilder, DynamicTextureAtlas, TextureHandle},
    prelude::*,
};
use winit::{
    dpi::LogicalSize,
    keyboard::{KeyCode, ModifiersState},
};

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
    let engine = GranularEngine::<Game>::new(LogicalSize::new(640, 480));
    engine.run();
}

#[include_wgsl_oil::include_wgsl_oil("../shaders/material.wgsl")]
pub mod material_shader {}

type MySimulation =
    Simulation<shader_types::MaterialName, shader_types::Material, shader_types::Cell>;

mod shader_types;

#[derive(Debug)]
struct Game {
    ctx: GeeseContextHandle<Self>,

    texture_handle: AssetHandle<TextureBundle>,
    texture2_handle: AssetHandle<TextureBundle>,

    material_tex_atlas: DynamicTextureAtlas,
    /// Stores if a new material texture was added and the atlas needs to be rebuilt
    material_atlas_dirty: bool,
    /// will be taken out on sim init
    materials_bg: Option<(granular::wgpu::BindGroup, granular::wgpu::BindGroupLayout)>,
}
impl Game {
    fn init(&mut self, _event: &events::Initialized) {
        {
            let mut camera = self.ctx.get_mut::<Camera>();
            camera.set_bottomleft_position(IVec2::new(0, 0));
            drop(camera);

            let (sand_tex, bg_tex, rock_tex) = {
                let mut asset_sys = self.ctx.get_mut::<AssetSystem>();
                (
                    asset_sys
                        .load(
                            asset_source!("../../assets/noita/sand.png"),
                            TextureBundleLoadSettings {
                                format: granular::wgpu::TextureFormat::Rgba8Unorm,
                                ..Default::default()
                            },
                        )
                        .unwrap(),
                    asset_sys
                        .load(
                            asset_source!("../../assets/noita/background_wandcave.png"),
                            TextureBundleLoadSettings {
                                format: granular::wgpu::TextureFormat::Rgba8Unorm,
                                ..Default::default()
                            },
                        )
                        .unwrap(),
                    asset_sys
                        .load(
                            asset_source!("../../assets/noita/rock.png"),
                            TextureBundleLoadSettings {
                                format: granular::wgpu::TextureFormat::Rgba8Unorm,
                                ..Default::default()
                            },
                        )
                        .unwrap(),
                )
            };
            self.add_material(
                shader_types::MaterialName::Empty,
                shader_types::Material {
                    tex_coords_start: Vec2::ZERO,
                    tex_coords_end: Vec2::ZERO,
                    color: vec4(0.0, 0.0, 0.0, 1.0),
                    density: 0.0,
                },
                Some(bg_tex),
            );
            self.add_material(
                shader_types::MaterialName::Sand,
                shader_types::Material {
                    tex_coords_start: Vec2::ZERO,
                    tex_coords_end: Vec2::ZERO,
                    color: vec4(1.0, 1.0, 0.0, 1.0),
                    density: 2.0,
                },
                Some(sand_tex),
            );
            self.add_material(
                shader_types::MaterialName::Water,
                shader_types::Material {
                    tex_coords_start: Vec2::ZERO,
                    tex_coords_end: Vec2::ZERO,
                    color: vec4(0.0, 0.0, 1.0, 1.0),
                    density: 1.0,
                },
                None,
            );
            self.add_material(
                shader_types::MaterialName::Rock,
                shader_types::Material {
                    tex_coords_start: Vec2::ZERO,
                    tex_coords_end: Vec2::ZERO,
                    color: vec4(0.2, 0.2, 0.2, 1.0),
                    density: 2.0,
                },
                Some(rock_tex),
            );

            let mut simulation = self.ctx.get_mut::<MySimulation>();
            simulation.init_simulation(
                granular::simulation::UserShaderInput {
                    main_shader: granular::simulation::include_file!("shaders/definitions.wgsl"),
                    includes: vec![
                        granular::simulation::include_file!("shaders/cell.wgsl"),
                        granular::simulation::include_file!("shaders/material.wgsl"),
                    ],
                },
                granular::simulation::UserShaderInput {
                    main_shader: granular::simulation::include_file!("shaders/display.wgsl"),
                    includes: vec![],
                },
                self.materials_bg.take().unwrap(),
            );

            let half_w = granular::simulation::GRID_WIDTH / 2;
            let quart_w = granular::simulation::GRID_WIDTH / 4;
            let third_h = granular::simulation::GRID_HEIGHT / 3;
            let fith_h = granular::simulation::GRID_HEIGHT / 5;
            for y in (third_h - fith_h)..(third_h + fith_h) {
                for x in (half_w - quart_w)..(half_w + quart_w) {
                    simulation.set_cell(
                        ivec2(x as i32, y as i32),
                        shader_types::Cell::new(
                            shader_types::MaterialName::Sand,
                            Vec2::ZERO,
                            Vec4::ONE,
                        ),
                    );
                }
            }

            for y in 40..47 {
                for x in 0..half_w {
                    simulation.set_cell(
                        ivec2(x as i32, y as i32),
                        shader_types::Cell::new(
                            shader_types::MaterialName::Rock,
                            Vec2::ZERO,
                            Vec4::ONE,
                        ),
                    );
                }
            }

            for y in (granular::simulation::GRID_HEIGHT - 3)..granular::simulation::GRID_HEIGHT {
                for x in 0..granular::simulation::GRID_WIDTH {
                    simulation.set_cell(
                        ivec2(x as i32, y as i32),
                        shader_types::Cell::new(
                            shader_types::MaterialName::Water,
                            Vec2::ZERO,
                            vec4(0.0, 0.0, 1.0, 1.0),
                        ),
                    );
                }
            }
        }
    }

    fn on_update(&mut self, _: &events::timing::FixedTick<16>) {
        let input = self.ctx.get::<InputSystem>();
        let vector = input.get_input_vector("cam_left", "cam_right", "cam_up", "cam_down");
        drop(input);
        let mut camera = self.ctx.get_mut::<Camera>();
        camera.translate(vector * 10);
        camera.set_bottomleft_position(IVec2::new(0, 0));
        let _pos = camera.position();
        drop(camera);
    }

    fn on_draw(&mut self, _: &granular::graphics::events::RecordGameRenderingCommands) {
        let mut renderer = self.ctx.get_mut::<BatchRenderer>();
        renderer.draw_quad_with_center(
            IVec2::new(0, 0),
            IVec2::new(10, 10),
            palette::named::RED,
            None,
            0,
        );
        renderer.draw_quad_with_center(
            IVec2::new(0, 250),
            IVec2::new(50, 50),
            palette::named::WHITE,
            Some(self.texture_handle.clone()),
            -2,
        );

        renderer.draw_quad_with_center(
            IVec2::new(50, 300),
            IVec2::new(50, 50),
            palette::named::WHITE,
            Some(self.texture2_handle.clone()),
            0,
        );
        drop(renderer);

        if self.material_atlas_dirty {
            let mut atlas_encoder = self
                .ctx
                .get_mut::<GraphicsSystem>()
                .device()
                .create_command_encoder(&granular::wgpu::CommandEncoderDescriptor {
                    label: Some("Simulation atlas command encoder"),
                });

            let asset_sys = self.ctx.get::<AssetSystem>();
            self.material_tex_atlas.rebuild_atlas(
                |handle| asset_sys.get(handle).unwrap().texture(),
                &mut atlas_encoder,
            );
            drop(asset_sys);
            {
                let mut graphics_sys = self.ctx.get_mut::<GraphicsSystem>();
                let context = graphics_sys.render_context();
                context.queue.submit(Some(atlas_encoder.finish()));
            }
            self.material_atlas_dirty = false;
        }
    }

    fn add_material(
        &mut self,
        material_name: shader_types::MaterialName,
        mut material_def: shader_types::Material,
        material_tex: Option<TextureHandle>,
    ) {
        let mut tex_coords_start = Vec2::ZERO;
        let mut tex_coords_end = Vec2::ZERO;
        if let Some(tex) = material_tex {
            let texture_size = {
                let asset_sys = self.ctx.get::<AssetSystem>();
                let tex = asset_sys.get(&tex).unwrap().texture();
                UVec2::new(tex.size().width, tex.size().height)
            };

            if !self.material_tex_atlas.contains_texture(&tex) {
                self.material_atlas_dirty = true;
                let res = self
                    .material_tex_atlas
                    .add_texture(tex.clone(), texture_size);
                if res.is_ok() {
                    (tex_coords_start, tex_coords_end) =
                        self.material_tex_atlas.get_texture_coords(&tex).unwrap();
                } else {
                    error!("Cannot insert material texture into atlas!");
                }
            }
        }
        material_def.tex_coords_start = tex_coords_start;
        material_def.tex_coords_end = tex_coords_end;

        let mut simulation = self.ctx.get_mut::<MySimulation>();
        simulation.add_material(material_name, material_def);
    }

    const EVENT_HANDLERS_SHARED: EventHandlers<Self> = event_handlers()
        .with(Self::init)
        .with(Self::on_update)
        .with(Self::on_draw);
}
impl GeeseSystem for Game {
    const EVENT_HANDLERS: EventHandlers<Self> = Self::EVENT_HANDLERS_SHARED;

    const DEPENDENCIES: Dependencies = dependencies()
        .with::<Mut<WindowSystem>>()
        .with::<Mut<InputSystem>>()
        .with::<Mut<Camera>>()
        .with::<Mut<GraphicsSystem>>()
        .with::<Mut<AssetSystem>>()
        .with::<Mut<BatchRenderer>>()
        .with::<Mut<MySimulation>>();

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
            // win_sys.set_window_size(winit::dpi::PhysicalSize::new(640, 480));
            win_sys.set_title("Granular engine testbed");
        }

        let graphics_sys = ctx.get::<GraphicsSystem>();
        let device = graphics_sys.device();
        let queue = graphics_sys.queue();
        let material_tex_atlas = DynamicTextureAtlas::new(
            device,
            queue,
            2048,
            2048,
            granular::wgpu::FilterMode::Nearest,
        );

        let (material_bgl, material_bg) = BindGroupBuilder::new()
            .add_binding_with_resource(
                1,
                granular::wgpu::ShaderStages::COMPUTE,
                granular::wgpu::BindingType::Texture {
                    sample_type: granular::wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: granular::wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                granular::wgpu::BindingResource::TextureView(material_tex_atlas.view()),
            )
            .add_binding_with_resource(
                2,
                granular::wgpu::ShaderStages::COMPUTE,
                granular::wgpu::BindingType::Sampler(granular::wgpu::SamplerBindingType::Filtering),
                granular::wgpu::BindingResource::Sampler(material_tex_atlas.sampler()),
            )
            .build("Materials bind group", device);
        let materials_bg = Some((material_bg, material_bgl));
        drop(graphics_sys);

        Self {
            ctx,
            texture_handle,
            texture2_handle,

            material_tex_atlas,
            material_atlas_dirty: false,
            materials_bg,
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
