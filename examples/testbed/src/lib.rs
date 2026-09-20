use std::{collections::VecDeque, path::PathBuf};

use fern::colors::{Color, ColoredLevelConfig};
#[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
use granular::filewatcher;
use granular::{
    graphics::{BindGroupBuilder, DynamicTextureAtlas, TextureHandle},
    prelude::*,
    simulation::prelude::*,
};
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
    let engine = GranularEngine::<Game>::new(UVec2::new(640, 480));
    engine.run();
}

type MySimulation =
    Simulation<shader_types::MaterialName, shader_types::Material, shader_types::Cell>;

mod shader_types;

enum MatColor {
    Tex(TextureHandle),
    Col(Vec4),
}

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

    #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
    shader_paths: Vec<PathBuf>,
}
impl Game {
    fn load_shaders(&mut self) -> VecDeque<granular::simulation::UserShaderInput> {
        let mut v = VecDeque::new();
        v.push_back(granular::simulation::UserShaderInput {
            main_shader: include_file!("shaders/definitions.wgsl"),
            includes: vec![
                include_file!("shaders/cell.wgsl"),
                include_file!("shaders/material.wgsl"),
            ],
        });
        v.push_back(granular::simulation::UserShaderInput {
            main_shader: include_file!("shaders/cell_logic.wgsl"),
            includes: vec![],
        });
        v.push_back(granular::simulation::UserShaderInput {
            main_shader: include_file!("shaders/display.wgsl"),
            includes: vec![],
        });

        #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
        {
            self.shader_paths.clear();
            for shader in &v {
                let path = PathBuf::from(shader.main_shader.path);
                self.ctx
                    .get_mut::<filewatcher::FileWatcher>()
                    .watch(&path, false);
                self.shader_paths.push(path);
                for dep in &shader.includes {
                    let path = PathBuf::from(dep.path);
                    self.ctx
                        .get_mut::<filewatcher::FileWatcher>()
                        .watch(&path, false);
                    self.shader_paths.push(path);
                }
            }
        }
        v
    }

    fn init(&mut self, _event: &events::Initialized) {
        {
            let mut camera = self.ctx.get_mut::<Camera>();
            camera.set_bottomleft_position(Vec2::ZERO);
            camera.set_motion(CameraMotion::SmoothPixel);
            camera.set_scaling_mode(ScalingMode::KeepAspect);
            drop(camera);

            let mut shaders = self.load_shaders();
            let mut simulation = self.ctx.get_mut::<MySimulation>();
            simulation.init_simulation(
                shaders.pop_front().unwrap(),
                shaders.pop_front().unwrap(),
                shaders.pop_front().unwrap(),
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
                        ivec2(x as i32, y),
                        shader_types::Cell::new(
                            shader_types::MaterialName::Rock,
                            Vec2::ZERO,
                            Vec4::ONE,
                        ),
                    );
                }
            }

            for y in 0..3 {
                for x in 0..granular::simulation::GRID_WIDTH {
                    simulation.set_cell(
                        ivec2(x as i32, y),
                        shader_types::Cell::new(
                            shader_types::MaterialName::Water,
                            Vec2::ZERO,
                            vec4(0.0, 0.0, 1.0, 1.0),
                        ),
                    );
                }
            }

            // const IMAGE_BYTES: &[u8] = include_bytes!("../../../assets/debug_body.png");
            const IMAGE_BYTES: &[u8] = include_bytes!("../../../assets/noita/brewing_stand.png");
            let _ = simulation.add_rigidbody(
                IMAGE_BYTES,
                shader_types::Cell::new(shader_types::MaterialName::Rock, Vec2::ZERO, Vec4::ONE),
            );
        }
    }

    #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
    fn on_filechange(&mut self, event: &filewatcher::events::FilesChanged) {
        for path in &event.paths {
            if self.shader_paths.contains(path) {
                let mut shaders = self.load_shaders();
                let mut simulation = self.ctx.get_mut::<MySimulation>();
                simulation.update_user_shaders(
                    shaders.pop_front().unwrap(),
                    shaders.pop_front().unwrap(),
                    shaders.pop_front().unwrap(),
                );
            }
        }
    }

    fn on_update(&mut self, _: &events::timing::FixedTick<16>) {
        let input = self.ctx.get::<InputSystem>();
        let vector = input.world_input_direction("cam_left", "cam_right", "cam_up", "cam_down");
        drop(input);
        let mut camera = self.ctx.get_mut::<Camera>();
        camera.translate(vector * 10.0);
        drop(camera);
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
        material_color: MatColor,
    ) {
        let texture_size = {
            match &material_color {
                MatColor::Tex(tex) => {
                    let asset_sys = self.ctx.get::<AssetSystem>();
                    let tex = asset_sys.get(tex).unwrap().texture();
                    UVec2::new(tex.size().width, tex.size().height)
                }
                MatColor::Col(_) => UVec2::ZERO,
            }
        };
        (material_def, self.material_atlas_dirty) = Self::prepare_material_definition(
            texture_size,
            &mut self.material_tex_atlas,
            material_def,
            material_color,
        );

        let mut simulation = self.ctx.get_mut::<MySimulation>();
        simulation.add_material(material_name, material_def);
    }

    /// Inserts the texture into the material atlas and stores if the atlas needs to be updated in the bool (second part of result)
    fn prepare_material_definition(
        texture_size: UVec2,
        material_tex_atlas: &mut DynamicTextureAtlas,
        mut material_def: shader_types::Material,
        material_color: MatColor,
    ) -> (shader_types::Material, bool) {
        let mut dirty = false;
        match material_color {
            MatColor::Tex(tex) => {
                if !material_tex_atlas.contains_texture(&tex) {
                    dirty = true;
                    let res = material_tex_atlas.add_texture(tex.clone(), texture_size);
                    if res.is_ok() {
                        (material_def.tex_coords_start, material_def.tex_coords_end) =
                            material_tex_atlas.get_texture_coords(&tex).unwrap();
                    } else {
                        error!("Cannot insert material texture into atlas!");
                    }
                }
            }
            MatColor::Col(col) => {
                material_def.color = col;
            }
        }
        (material_def, dirty)
    }

    const EVENT_HANDLERS_SHARED: EventHandlers<Self> = event_handlers()
        .with(Self::init)
        .with(Self::on_update)
        .with(Self::on_draw);

    const SHARED_DEPENDENCIES: Dependencies = dependencies()
        .with::<Mut<WindowSystem>>()
        .with::<Mut<InputSystem>>()
        .with::<Mut<Camera>>()
        .with::<Mut<GraphicsSystem>>()
        .with::<Mut<AssetSystem>>()
        .with::<Mut<BatchRenderer>>()
        .with::<Mut<MySimulation>>();
}
impl GeeseSystem for Game {
    #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
    const EVENT_HANDLERS: EventHandlers<Self> =
        Self::EVENT_HANDLERS_SHARED.with(Self::on_filechange);
    #[cfg(any(target_arch = "wasm32", not(debug_assertions)))]
    const EVENT_HANDLERS: EventHandlers<Self> = Self::EVENT_HANDLERS_SHARED;

    #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
    const DEPENDENCIES: Dependencies =
        Self::SHARED_DEPENDENCIES.with::<Mut<filewatcher::FileWatcher>>();
    #[cfg(any(target_arch = "wasm32", not(debug_assertions)))]
    const DEPENDENCIES: Dependencies = Self::SHARED_DEPENDENCIES;

    fn new(mut ctx: GeeseContextHandle<Self>) -> Self {
        info!("Game created");

        let graphics_sys = ctx.get::<GraphicsSystem>();
        let device = graphics_sys.device();
        let queue = graphics_sys.queue();
        let mut material_tex_atlas = DynamicTextureAtlas::new(
            "Testbed material texture atlas",
            device,
            queue,
            2048,
            2048,
            granular::wgpu::FilterMode::Nearest,
        );
        drop(graphics_sys);

        {
            let (sand_tex, bg_tex, rock_tex) = {
                let mut asset_sys = ctx.get_mut::<AssetSystem>();

                let mut load_tex = |source| {
                    let handle = asset_sys
                        .load::<TextureBundle>(
                            source,
                            TextureBundleLoadSettings {
                                format: granular::wgpu::TextureFormat::Rgba8Unorm,
                                ..Default::default()
                            },
                        )
                        .unwrap();
                    let texture_size = {
                        let tex = asset_sys.get(&handle).unwrap().texture();
                        UVec2::new(tex.size().width, tex.size().height)
                    };
                    (handle, texture_size)
                };

                (
                    load_tex(asset_source!("../../assets/noita/sand.png")),
                    load_tex(asset_source!("../../assets/noita/background_wandcave.png")),
                    load_tex(asset_source!("../../assets/noita/rock.png")),
                )
            };

            let mut sim = ctx.get_mut::<MySimulation>();
            sim.add_material(
                shader_types::MaterialName::Empty,
                Self::prepare_material_definition(
                    bg_tex.1,
                    &mut material_tex_atlas,
                    shader_types::Material::new(0.0),
                    MatColor::Tex(bg_tex.0),
                )
                .0,
            );
            sim.add_material(
                shader_types::MaterialName::Sand,
                Self::prepare_material_definition(
                    sand_tex.1,
                    &mut material_tex_atlas,
                    shader_types::Material::new(2.0),
                    MatColor::Tex(sand_tex.0),
                )
                .0,
            );
            sim.add_material(
                shader_types::MaterialName::Water,
                Self::prepare_material_definition(
                    UVec2::ZERO,
                    &mut material_tex_atlas,
                    shader_types::Material::new(1.0),
                    MatColor::Col(vec4(0.0, 0.0, 1.0, 1.0)),
                )
                .0,
            );
            sim.add_material(
                shader_types::MaterialName::Rock,
                Self::prepare_material_definition(
                    rock_tex.1,
                    &mut material_tex_atlas,
                    shader_types::Material::new(5.0),
                    MatColor::Tex(rock_tex.0),
                )
                .0,
            );
        }
        let material_atlas_dirty = true;

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

        let graphics_sys = ctx.get::<GraphicsSystem>();
        let device = graphics_sys.device();

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
            material_atlas_dirty,
            materials_bg,

            #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
            shader_paths: vec![],
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
