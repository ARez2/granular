#![feature(trait_alias)]

use encase::{ShaderType, UniformBuffer};
use glam::prelude::*;
use granular_core::{
    filewatcher::{self, FileWatcher},
    graphics::BindGroupBuilder,
    prelude::*,
};
use rapier2d::dynamics::{RigidBodyBuilder, RigidBodyHandle};
use rustc_hash::FxHashMap as HashMap;
#[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
use std::path::PathBuf;
use std::{borrow::Cow, fmt::Display};
use web_time::{Duration, Instant};
use wgpu::{Buffer, util::DeviceExt};
use wgsl_preprocessor::include_file;

pub mod prelude {
    pub use super::{AdditionalMatNameFlags, CellStruct, UserShaderInput};
    pub use num_enum::IntoPrimitive;
    pub use proc_macros::MatName;
    pub use strum;
    pub use wgsl_preprocessor::include_file;
}

#[doc(hidden)]
pub mod __macro_support {
    pub use num_enum;
    pub use strum;
}

pub const GRID_WIDTH: u32 = 128;
pub const GRID_HEIGHT: u32 = 128;

mod shader_types;
mod sim_helpers;
use shader_types::{MaybeCell, RB, RBCell};
use sim_helpers::*;
mod sim_physics;
use sim_physics::SimPhysics;

use crate::shader_types::{
    MAYBECELL_FLAG_IS_SOME, RBCELL_FLAG_PIXELSCENE_COLOR, RBCELL_FLAG_VALID,
};

/// Is automatically implemented when you add `#[MatName]` (from `granular::simulation::prelude`) to your material name enum
pub trait MatName:
    'static
    + core::fmt::Debug
    + Display
    + Clone
    + Copy
    + core::hash::Hash
    + PartialEq
    + Eq
    + strum::IntoEnumIterator
    + Into<u32>
    + From<u32>
    + AdditionalMatNameFlags
{
}

/// Needs to be implemented for your MatName enum
pub trait AdditionalMatNameFlags {
    fn has_collision(&self) -> bool;
}
const FLAG_METHODS: &[FlagMethod] = &[FlagMethod {
    name: "has_collision",
    call: |x| x.has_collision(),
}];
struct FlagMethod {
    name: &'static str,
    call: fn(&dyn AdditionalMatNameFlags) -> bool,
}

/// Needs `#[derive(ShaderType, Clone)]` and a `Default` implementation
pub trait MaterialShaderStruct = 'static
    + Default
    + Clone
    + encase::ShaderType
    + encase::ShaderSize
    + encase::internal::WriteInto;

pub trait CellStruct:
    Sized
    + 'static
    + Default
    + Clone
    + Copy
    + encase::ShaderType
    + encase::ShaderSize
    + encase::internal::WriteInto
{
    fn material_name(&self) -> u32;
    fn set_color(&mut self, new_color: Vec4);
}

/// Use this to pass your shaders to the simulation in `Simulation::init_simulation` or `Simulation::update_user_shaders`
#[derive(Debug, Clone)]
pub struct UserShaderInput {
    /// This is the root shader which includes the other `includes`
    pub main_shader: wgsl_preprocessor::IncludedFile,
    /// All the shader (even nested!) which get included by the `main_shader`
    pub includes: Vec<wgsl_preprocessor::IncludedFile>,
}

struct SimulationPass {
    #[allow(unused)]
    name: String,
    pipeline: wgpu::ComputePipeline,
    dispatch_size: (u32, u32, u32),
}
impl SimulationPass {
    fn new(
        name: impl Into<String>,
        pipeline: wgpu::ComputePipeline,
        dispatch_size: (u32, u32, u32),
    ) -> Self {
        Self {
            name: name.into(),
            pipeline,
            dispatch_size,
        }
    }
}

/// A falling sand simulation framework. Call `init_simulation` ASAP to initialize the simulation! Otherwise it will not run!
pub struct Simulation<N: MatName, M: MaterialShaderStruct, C: CellStruct> {
    ctx: GeeseContextHandle<Self>,
    pub frame: u64,
    pub tickrate: Duration,
    last_tick: Instant,
    accumulator: Duration,

    /// One simulation pixel will be `display_scale`-many pixels on screen
    display_scale: f32,

    /// Buffer which gets written from the CPU (and basically stores the GPU equivalent of Option<Cell>)
    cells_cpu_buffer: Box<[MaybeCell<C>]>,
    /// Stores which indices of `cells_cpu_buffer` have changed
    cells_dirty_indices: Vec<usize>,
    /// GPU buffer into which `cells_cpu_buffer` will get uploaded
    cpu_to_gpu_buffer: wgpu::Buffer,

    /// Shader which **must** contain a definition for `struct Material` and `struct Cell` (with those names)
    user_definitions_shader: Option<UserShaderInput>,
    /// Shader which processes all the cells
    user_cell_process_shader: Option<UserShaderInput>,
    /// Shader, which is run to display the sim
    user_display_shader: Option<UserShaderInput>,

    /// Is initialized after the user calls init
    simulation_passes: Vec<SimulationPass>,
    #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
    shader_paths: Vec<PathBuf>,
    compute_pl_layout: Option<wgpu::PipelineLayout>,

    /// The shared bind group layout for the first bindgroup
    sim_bgl_a_b: wgpu::BindGroupLayout,
    /// The Ping-part of the ping pong bindgroup (stuff which changes each frame)
    sim_bind_group1_a: wgpu::BindGroup,
    /// The Pong-part of the ping pong bindgroup (stuff which changes each frame)
    sim_bind_group1_b: wgpu::BindGroup,
    /// Bindgroup for debug purposes
    debug_bind_group: (wgpu::BindGroup, wgpu::BindGroupLayout),
    /// Bindgroup which contains stuff like display texture or materials (which dont change every frame)
    sim_bind_group2: (wgpu::BindGroup, wgpu::BindGroupLayout),
    /// The bindgroup that the user can provide for display etc.
    user_bind_group: Option<(wgpu::BindGroup, wgpu::BindGroupLayout)>,

    params: shader_types::Params,
    params_bytes: [u8; size_of::<shader_types::Params>()],
    params_buffer: wgpu::Buffer,

    /// The handle to the texture which stores the color output of the simulation
    display_tex_handle: AssetHandle<TextureBundle>,
    /// The handle to the texture which stores debug stuff of the simulation
    debug_tex_handle: AssetHandle<TextureBundle>,

    /// Maps the enum values to an index in `materials`
    material_names: HashMap<N, usize>,
    /// Contains the materials, like they would be laid out in GPU memory
    materials: Vec<Option<M>>,
    /// The GPU memory containing the materials
    materials_ssbo: Buffer,

    /// The wrapper around Rapier2D
    physics: SimPhysics,
    rb_cells_cpu: Box<[RBCell<C>]>,
    /// GPU storage for the RBCell's
    rb_cells_buffer: wgpu::Buffer,
    /// CPU side storage of the GPU representation of Rigidbodies
    rbs: Vec<RB>,
    /// GPU storage for the RB's
    rbs_buffer: wgpu::Buffer,
    /// Maps a rapier RigidBodyHandle to an index into rbs
    rapier_rb_to_sim_rb: HashMap<RigidBodyHandle, usize>,
}
impl<N: MatName, M: MaterialShaderStruct, C: CellStruct> Simulation<N, M, C> {
    fn update(&mut self, _: &granular_core::graphics::events::RunSimulation) {
        if self.simulation_passes.is_empty() {
            return;
        }
        let mut graphics_sys = self.ctx.get_mut::<GraphicsSystem>();
        let context = graphics_sys.render_context();

        let now = Instant::now();
        let dt = now - self.last_tick;
        self.accumulator += dt;

        while self.accumulator >= self.tickrate {
            #[cfg(feature = "trace")]
            profiling::scope!("accumulator");

            self.params.tick = self.frame as u32;

            for pass in &self.simulation_passes {
                #[cfg(feature = "trace")]
                profiling::scope!("compute pass");

                #[cfg(feature = "trace")]
                let mut compute_pass = profiler_scope.scoped_compute_pass(pass.name);
                #[cfg(not(feature = "trace"))]
                let mut compute_pass =
                    context
                        .encoder
                        .begin_compute_pass(&wgpu::ComputePassDescriptor {
                            label: Some("compute pass"),
                            timestamp_writes: None,
                        });
                compute_pass.set_pipeline(&pass.pipeline);
                if self.frame.is_multiple_of(2) {
                    compute_pass.set_bind_group(0, &self.sim_bind_group1_a, &[]);
                } else {
                    compute_pass.set_bind_group(0, &self.sim_bind_group1_b, &[]);
                }
                compute_pass.set_bind_group(1, &self.sim_bind_group2.0, &[]);
                if let Some(user_bg) = &self.user_bind_group {
                    compute_pass.set_bind_group(2, &user_bg.0, &[]);
                }
                compute_pass.set_bind_group(4, &self.debug_bind_group.0, &[]);
                compute_pass.dispatch_workgroups(
                    pass.dispatch_size.0,
                    pass.dispatch_size.1,
                    pass.dispatch_size.2,
                );
            }
            self.accumulator -= self.tickrate;
            self.frame += 1;
        }

        let mut writer = UniformBuffer::new(&mut self.params_bytes);
        let _ = writer.write(&self.params);
        context
            .queue
            .write_buffer(&self.params_buffer, 0, &self.params_bytes);
        if !dt.is_zero() {
            self.last_tick = Instant::now();
        }
        drop(graphics_sys);
    }

    fn on_game_render(&mut self, _: &graphics::events::RecordGameRenderingCommands) {
        {
            let mut graphics_sys = self.ctx.get_mut::<GraphicsSystem>();
            let context = graphics_sys.render_context();
            {
                let default_mat = M::default();
                let mut materials_buffer = encase::StorageBuffer::new(Vec::<u8>::new());
                let mats = self
                    .materials
                    .clone()
                    .iter_mut()
                    .map(|v| {
                        if v.is_none() {
                            default_mat.clone()
                        } else {
                            v.clone().unwrap()
                        }
                    })
                    .collect::<Vec<M>>();
                let _ = materials_buffer.write(&mats);
                context
                    .queue
                    .write_buffer(&self.materials_ssbo, 0, &materials_buffer.into_inner());
            }

            if !self.cells_dirty_indices.is_empty() {
                let mut staging = context
                    .queue
                    .write_buffer_with(&self.cpu_to_gpu_buffer, 0, self.cells_cpu_buffer.size())
                    .expect("Invalid buffer write");

                encase::StorageBuffer::new(EncaseStaging(&mut staging))
                    .write(self.cells_cpu_buffer.as_ref())
                    .expect("encase serialization failed");

                for idx in &self.cells_dirty_indices {
                    // set is_some to false
                    self.cells_cpu_buffer[*idx].flags &= !MAYBECELL_FLAG_IS_SOME;
                }
                self.cells_dirty_indices.clear();
            }
            {
                // Write updated Rigidbody transforms to the sim buffer
                let mut staging = context
                    .queue
                    .write_buffer_with(&self.rbs_buffer, 0, self.rbs.size())
                    .expect("Invalid buffer write");
                encase::StorageBuffer::new(EncaseStaging(&mut staging))
                    .write(&self.rbs)
                    .expect("encase serialization failed");
            }
            {
                // Write RBCell buffer to the sim buffer
                let mut staging = context
                    .queue
                    .write_buffer_with(&self.rb_cells_buffer, 0, self.rb_cells_cpu.size())
                    .expect("Invalid buffer write");
                encase::StorageBuffer::new(EncaseStaging(&mut staging))
                    .write(&self.rb_cells_cpu)
                    .expect("encase serialization failed");
            }
        }

        {
            let mut renderer = self.ctx.get_mut::<BatchRenderer>();
            let size = Vec2::new(GRID_WIDTH as f32, GRID_HEIGHT as f32) * self.display_scale;
            renderer.draw_quad_with_bottomleft(
                Vec2::ZERO,
                size,
                0.0,
                palette::named::WHITE,
                QuadTex::Texture(self.display_tex_handle.clone()),
                -10,
                DrawSpace::World,
            );
            renderer.mark_quad_texture_dirty(self.display_tex_handle.clone());
            renderer.draw_quad_with_bottomleft(
                Vec2::ZERO,
                size,
                0.0,
                palette::named::WHITE,
                QuadTex::Texture(self.debug_tex_handle.clone()),
                -9,
                DrawSpace::World,
            );
            renderer.mark_quad_texture_dirty(self.debug_tex_handle.clone());
        }
    }

    fn on_display_game_render(&mut self, _: &graphics::events::RecordUiRenderingCommands) {
        let disp_scale = self.display_scale;
        let mut debug = self.ctx.get_mut::<DebugDraw>();

        let color = vec4(0.0, 1.0, 0.2, 0.8);
        let thickness = 0.2 * disp_scale;
        let layer = 100;
        let draw_space = DrawSpace::World;

        self.physics.draw_each_collider(
            &mut *debug,
            |debug, center, size, angle| {
                debug.draw_rect_center(
                    center * disp_scale,
                    size * disp_scale,
                    angle,
                    color,
                    thickness,
                    layer,
                    draw_space,
                );
            },
            |debug, center, radius| {
                debug.draw_circle(
                    center * disp_scale,
                    radius * disp_scale,
                    color,
                    layer,
                    draw_space,
                );
            },
            |debug, points| {
                let scaled: Vec<Vec2> = points.iter().map(|&point| point * disp_scale).collect();

                debug.draw_polyline(&scaled, color, thickness, layer, draw_space);
            },
        );
    }

    fn fixed_step(&mut self, _: &crate::events::timing::FixedTick<16>) {
        self.physics.step();

        for (rb_handle, rb_idx) in &self.rapier_rb_to_sim_rb {
            let pose = self.physics.get_rigidbody_pose(*rb_handle);
            // debug!("pos: {}", pose.0);
            self.rbs[*rb_idx].position = pose.0;
            self.rbs[*rb_idx].angle_degrees = pose.1.to_degrees();
            // self.rbs[*rb_idx].angle_degrees = (self.frame as f32 / 100.0).to_degrees();
        }
    }

    /// Call this to initialize the simulation. Before calling this, the simulation will not run!
    /// `definitions_shader` **must** contain a definition for `struct Material` and `struct Cell` (with those names)
    pub fn init_simulation(
        &mut self,
        definitions_shader: UserShaderInput,
        cell_process_shader: UserShaderInput,
        display_shader: UserShaderInput,
        bindgroup: (wgpu::BindGroup, wgpu::BindGroupLayout),
    ) {
        self.user_definitions_shader = Some(definitions_shader);
        self.user_cell_process_shader = Some(cell_process_shader);
        self.user_display_shader = Some(display_shader);
        self.user_bind_group = Some(bindgroup);
        self.rebuild_pipelines();
    }

    pub fn update_user_shaders(
        &mut self,
        definitions_shader: UserShaderInput,
        cell_process_shader: UserShaderInput,
        display_shader: UserShaderInput,
    ) {
        self.user_definitions_shader = Some(definitions_shader);
        self.user_cell_process_shader = Some(cell_process_shader);
        self.user_display_shader = Some(display_shader);
    }

    /// Inserts a new material definition into the simulation
    pub fn add_material(&mut self, material_name: N, material_def: M) {
        // Finds the first None index in self.materials and inserts the material_def there.
        // If nothing is free, inserts material_def at the end of self.materials
        let index = if let Some((i, slot)) = self
            .materials
            .iter_mut()
            .enumerate()
            .find(|(_, mat)| mat.is_none())
        {
            *slot = Some(material_def);
            i
        } else {
            let i = self.materials.len();
            self.materials.push(Some(material_def));
            i
        };
        self.material_names.insert(material_name, index);
    }

    pub fn add_rigidbody(&mut self, rb_image_bytes: &[u8], filler_cell: C) {
        let mut image =
            image::load_from_memory(rb_image_bytes).expect("failed to decode embedded image");
        image
            .set_color_space(image::metadata::Cicp::SRGB)
            .expect("failed to set srgb color space");
        image
            .convert_color_space(
                image::metadata::Cicp::SRGB_LINEAR,
                image::ConvertColorOptions::default(),
                image::ColorType::Rgba32F,
            )
            .expect("failed to convert image to linear sRGB");
        let image = image.to_rgba8();

        let mut last_idx = 0;
        let mut collider_pixels = vec![];

        let img_center = ivec2(image.width() as i32 / 2, image.height() as i32 / 2);

        for (idx, (x, y, pixel)) in image.enumerate_pixels().enumerate() {
            #[allow(unused)]
            let [r, g, b, a] = pixel.0;

            let flags = RBCELL_FLAG_VALID | RBCELL_FLAG_PIXELSCENE_COLOR;

            let mut inner_cell = if a == 0 { C::default() } else { filler_cell };
            inner_cell.set_color(vec4(r as f32, g as f32, b as f32, a as f32) / 255.0);

            let pix_local_pos_in_rb = ivec2(x as i32 - img_center.x, img_center.y - y as i32);

            let matname: N = inner_cell.material_name().into();
            if a != 0 && matname.has_collision() {
                collider_pixels.push(pix_local_pos_in_rb);
            }

            self.rb_cells_cpu[idx] = RBCell {
                inner_cell,
                rb_local_pos: pix_local_pos_in_rb,
                rb_index: 0,
                flags,
            };
            last_idx = idx;
        }

        let position = vec2(50.0, 30.0);
        let rb_handle = self.physics.create_rigidbody(
            RigidBodyBuilder::dynamic(),
            position,
            (0.0f32).to_radians(),
            &collider_pixels,
        );

        self.rapier_rb_to_sim_rb.insert(rb_handle, 0);
        let mut center_of_mass = Vec2::ZERO;
        for pt in &collider_pixels {
            center_of_mass += pt.as_vec2();
        }
        center_of_mass /= collider_pixels.len() as f32;

        self.rbs.push(RB {
            position,
            center_of_mass,
            angle_degrees: 0.0,
            rbcells_start: 0,
            rbcells_end: last_idx as u32,
        });
    }

    #[inline(always)]
    fn pos_to_idx(&self, pos: IVec2) -> usize {
        (pos.y * GRID_WIDTH as i32 + pos.x) as usize
    }

    pub fn set_cell(&mut self, pos: IVec2, cell: C) {
        let idx = self.pos_to_idx(pos);
        self.cells_cpu_buffer[idx].inner_cell = cell;
        self.cells_cpu_buffer[idx].flags = MAYBECELL_FLAG_IS_SOME;
        self.cells_dirty_indices.push(idx);
    }

    #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
    fn on_filechange(&mut self, event: &filewatcher::events::FilesChanged) {
        for path in &event.paths {
            if self.shader_paths.contains(path) {
                self.rebuild_pipelines();
            }
        }
    }

    fn rebuild_pipelines(&mut self) {
        self.compute_pl_layout = Some(
            self.ctx
                .get::<GraphicsSystem>()
                .device()
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("compute pipeline layout"),
                    bind_group_layouts: &[
                        Some(&self.sim_bgl_a_b),
                        Some(&self.sim_bind_group2.1),
                        self.user_bind_group.as_ref().map(|v| &v.1),
                        None,
                        Some(&self.debug_bind_group.1),
                    ],
                    immediate_size: 0,
                }),
        );
        self.build_compute_shader();
    }

    /// Assembles the compute shader and returns the shader source as well as a list of dependencies
    fn build_compute_shader(&mut self) {
        let Some(definitions_shader) = self.user_definitions_shader.clone() else {
            error!("User definitions shader not set!");
            return;
        };
        let Some(process_shader) = self.user_cell_process_shader.clone() else {
            error!("User cell process shader not set!");
            return;
        };
        let Some(display_shader) = self.user_display_shader.clone() else {
            error!("User display shader not set!");
            return;
        };

        let mut all_dependencies = vec![];
        #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
        let shader_dump_dir = std::env::current_exe()
            .unwrap()
            .with_file_name("shader-build");
        #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
        std::fs::create_dir_all(&shader_dump_dir).unwrap();

        let definitions_shader_res = wgsl_preprocessor::Preprocessor::new(
            definitions_shader.main_shader,
            definitions_shader.includes,
        )
        .build();
        if let Err(e) = definitions_shader_res {
            error!("Error while preprocessing users definition shader: {}", e);
            return;
        }
        let mut definitions_shader = definitions_shader_res.unwrap();
        all_dependencies.push(definitions_shader.dependencies.clone());

        let mut flagmethods_sources = HashMap::default();
        for method in FLAG_METHODS {
            let signature = format!(
                "fn matname_{}(material: u32) -> bool {{\nswitch material {{\n",
                method.name
            );
            flagmethods_sources.insert(method.name, signature);
        }

        let mut material_constants = String::new();
        for mat_name in N::iter() {
            let mat_idx: u32 = mat_name.into();
            let matname_string = format!("MAT_{}", mat_name.to_string().to_uppercase());
            let const_mat_wgsl_string = format!("const {}: u32 = {};\n", matname_string, mat_idx);
            material_constants.push_str(&const_mat_wgsl_string);

            for method in FLAG_METHODS {
                let flag_value = (method.call)(&mat_name);
                // I know this is the same formatting Rust uses, but I want to be explicit
                let flag_value_str = if flag_value { "true" } else { "false" };
                let case_string = format!(
                    "case {} {{\nreturn {};\n}}\n",
                    matname_string, flag_value_str
                );
                let source = flagmethods_sources.get_mut(&method.name).unwrap();
                source.push_str(&case_string);
            }
        }
        material_constants.push('\n');

        let mut combined_flag_sources = String::new();
        for method in FLAG_METHODS {
            let source = flagmethods_sources.get_mut(&method.name).unwrap();
            source.push_str("case default {\nreturn false;\n}\n");
            source.push_str("}}\n\n");
            combined_flag_sources.push_str(source);
        }

        let inserted_source = format!("{}\n\n{}", material_constants, combined_flag_sources);
        definitions_shader.source.insert_str(0, &inserted_source);

        // Definitions shader is the first shader, so just validate it alone
        if let Err(e) = validate_wgsl(&definitions_shader.source) {
            error!(
                "Error while validating user definitions shader with inserted material name constants: {}",
                e
            );
            #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
            {
                let dumppath = shader_dump_dir.join("user_definitions.wgsl");
                let _ = std::fs::write(&dumppath, definitions_shader.source);
                error!("Shader source written to: {}", dumppath.display());
            }
            return;
        }

        let process_shader_res = wgsl_preprocessor::Preprocessor::new(
            process_shader.main_shader,
            process_shader.includes,
        )
        .build();
        if let Err(e) = process_shader_res {
            error!("Error while preprocessing users cell process shader: {}", e);
            return;
        }
        let process_shader = process_shader_res.unwrap();
        all_dependencies.push(process_shader.dependencies.clone());

        // Since the processing shader can use stuff before it, we need to make sure that stuff exists
        let mut compute_to_process_preprocessor = wgsl_preprocessor::Preprocessor::new(
            include_file!("shaders/compute_to_1process.wgsl"),
            vec![
                include_file!("shaders/shared.wgsl"),
                include_file!("shaders/debug_print.wgsl"),
                include_file!("shaders/blend_modes.wgsl"),
                include_file!("shaders/actions.wgsl"),
            ],
        );
        compute_to_process_preprocessor.define_value("GRID_WIDTH", GRID_WIDTH);
        compute_to_process_preprocessor.define_value("GRID_HEIGHT", GRID_HEIGHT);
        compute_to_process_preprocessor
            .define_value("USER_DEFINITIONS_SHADER", definitions_shader.source);
        compute_to_process_preprocessor
            .define_value("USER_CELL_PROCESS_SHADER", process_shader.source);
        let compute_to_process_res = compute_to_process_preprocessor.build();
        if let Err(e) = compute_to_process_res {
            error!(
                "Error while preprocessing simulation shader until (incl.) users cell process shader: {}",
                e
            );
            return;
        }
        let compute_to_process_shader = compute_to_process_res.unwrap();
        all_dependencies.push(compute_to_process_shader.dependencies.clone());
        if let Err(e) = validate_wgsl(&compute_to_process_shader.source) {
            error!(
                "Error while validating simulation shader until (incl.) users cell process shader: {}",
                e
            );
            #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
            {
                let dumppath = shader_dump_dir.join("cell_process_shader.wgsl");
                let _ = std::fs::write(&dumppath, compute_to_process_shader.source);
                error!("Shader source written to: {}", dumppath.display());
            }
            return;
        }

        let display_shader_res = wgsl_preprocessor::Preprocessor::new(
            display_shader.main_shader,
            display_shader.includes,
        )
        .build();
        if let Err(e) = display_shader_res {
            error!("Error while preprocessing users display shader: {}", e);
            return;
        }
        let display_shader = display_shader_res.unwrap();
        all_dependencies.push(display_shader.dependencies.clone());

        let mut compute_to_display_preprocessor = wgsl_preprocessor::Preprocessor::new(
            include_file!("shaders/compute_to_2display.wgsl"),
            vec![],
        );
        compute_to_display_preprocessor.define_value("USER_DISPLAY_SHADER", display_shader.source);
        let compute_to_display_res = compute_to_display_preprocessor.build();
        if let Err(e) = compute_to_display_res {
            error!(
                "Error while preprocessing simulation shader until (incl.) users display shader: {}",
                e
            );
            return;
        }
        let compute_to_display_shader = compute_to_display_res.unwrap();
        all_dependencies.push(compute_to_display_shader.dependencies.clone());

        let process_and_display_shader_src = format!(
            "{}\n\n{}",
            compute_to_process_shader.source, compute_to_display_shader.source
        );
        if let Err(e) = validate_wgsl(&process_and_display_shader_src) {
            error!(
                "Error while validating simulation shader until (incl.) users display shader: {}",
                e
            );
            #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
            {
                let dumppath = shader_dump_dir.join("display_shader.wgsl");
                let _ = std::fs::write(&dumppath, process_and_display_shader_src);
                error!("Shader source written to: {}", dumppath.display());
            }
            return;
        }

        let mut compute_shader = wgsl_preprocessor::Preprocessor::new(
            include_file!("shaders/compute_to_3end.wgsl"),
            vec![],
        );
        let compute_shader_res = compute_shader.build();
        if let Err(e) = compute_shader_res {
            error!(
                "Error while preprocessing simulation compute to the end shader: {}",
                e
            );
            return;
        }
        let compute_shader = compute_shader_res.unwrap();
        all_dependencies.push(compute_shader.dependencies.clone());

        let full_shader_source = format!(
            "{}\n\n{}",
            process_and_display_shader_src, compute_shader.source
        );

        #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
        {
            self.shader_paths = all_dependencies.into_iter().flatten().collect();
            for path in &self.shader_paths {
                self.ctx.get_mut::<FileWatcher>().watch(path, true);
            }
        }

        #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
        {
            let dumppath = shader_dump_dir.join("full_simulation_shader.wgsl");
            let _ = std::fs::write(&dumppath, &full_shader_source);
        }

        let graphics_sys = self.ctx.get::<GraphicsSystem>();
        let device = graphics_sys.device();
        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Simulation main compute shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Owned(full_shader_source)),
        });
        self.simulation_passes = Self::create_simulation_passes(
            device,
            self.compute_pl_layout.as_ref().unwrap(),
            &shader_module,
        )
    }

    fn create_simulation_passes(
        device: &wgpu::Device,
        layout: &wgpu::PipelineLayout,
        compute_shader: &wgpu::ShaderModule,
    ) -> Vec<SimulationPass> {
        let full_grid_dispatch = (GRID_WIDTH.div_ceil(8), GRID_HEIGHT.div_ceil(8), 1);
        let num_rbcells = Self::total_nr_cells();
        let num_rbcell_workgroups = num_rbcells.div_ceil(256);
        let rbcell_dispatch = (num_rbcell_workgroups as u32, 1, 1);

        vec![
            SimulationPass::new(
                "prepare compute pass",
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("prepare compute pipeline"),
                    layout: Some(layout),
                    module: compute_shader,
                    entry_point: Some("prepare"),
                    compilation_options: Default::default(),
                    cache: None,
                }),
                full_grid_dispatch,
            ),
            SimulationPass::new(
                "insert bodies compute pass",
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("insert bodies compute pipeline"),
                    layout: Some(layout),
                    module: compute_shader,
                    entry_point: Some("insert_bodies"),
                    compilation_options: Default::default(),
                    cache: None,
                }),
                rbcell_dispatch,
            ),
            SimulationPass::new(
                "compose grid compute pass",
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("compose grid compute pipeline"),
                    layout: Some(layout),
                    module: compute_shader,
                    entry_point: Some("compose_grid"),
                    compilation_options: Default::default(),
                    cache: None,
                }),
                full_grid_dispatch,
            ),
            SimulationPass::new(
                "propose compute pass",
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("propose compute pipeline"),
                    layout: Some(layout),
                    module: compute_shader,
                    entry_point: Some("propose"),
                    compilation_options: Default::default(),
                    cache: None,
                }),
                full_grid_dispatch,
            ),
            SimulationPass::new(
                "resolve compute pass",
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("resolve compute pipeline"),
                    layout: Some(layout),
                    module: compute_shader,
                    entry_point: Some("resolve"),
                    compilation_options: Default::default(),
                    cache: None,
                }),
                full_grid_dispatch,
            ),
            SimulationPass::new(
                "commit compute pass",
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("commit compute pipeline"),
                    layout: Some(layout),
                    module: compute_shader,
                    entry_point: Some("commit"),
                    compilation_options: Default::default(),
                    cache: None,
                }),
                full_grid_dispatch,
            ),
            SimulationPass::new(
                "display compute pass",
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("display compute pipeline"),
                    layout: Some(layout),
                    module: compute_shader,
                    entry_point: Some("display"),
                    compilation_options: Default::default(),
                    cache: None,
                }),
                full_grid_dispatch,
            ),
            SimulationPass::new(
                "extract bodies compute pass",
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("extract bodies compute pipeline"),
                    layout: Some(layout),
                    module: compute_shader,
                    entry_point: Some("extract_bodies"),
                    compilation_options: Default::default(),
                    cache: None,
                }),
                full_grid_dispatch,
            ),
        ]
    }

    fn create_maybecell(cell: Option<C>) -> shader_types::MaybeCell<C> {
        match cell {
            Some(c) => shader_types::MaybeCell {
                inner_cell: c,
                flags: MAYBECELL_FLAG_IS_SOME,
            },
            None => shader_types::MaybeCell {
                inner_cell: C::default(),
                flags: 0,
            },
        }
    }

    const fn total_nr_cells() -> usize {
        (GRID_WIDTH * GRID_HEIGHT) as usize
    }
}
impl<N: MatName, M: MaterialShaderStruct, C: CellStruct> GeeseSystem for Simulation<N, M, C> {
    const DEPENDENCIES: Dependencies = dependencies()
        .with::<Mut<GraphicsSystem>>()
        .with::<Mut<BatchRenderer>>()
        .with::<Mut<DebugDraw>>()
        .with::<Camera>()
        .with::<Mut<AssetSystem>>()
        .with::<Mut<FileWatcher>>();

    #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
    const EVENT_HANDLERS: EventHandlers<Self> = event_handlers()
        .with(Self::update)
        .with(Self::on_game_render)
        .with(Self::on_display_game_render)
        .with(Self::fixed_step)
        .with(Self::on_filechange);
    #[cfg(any(target_arch = "wasm32", not(debug_assertions)))]
    const EVENT_HANDLERS: EventHandlers<Self> = event_handlers()
        .with(Self::update)
        .with(Self::on_game_render)
        .with(Self::on_display_game_render)
        .with(Self::fixed_step);

    fn new(mut ctx: GeeseContextHandle<Self>) -> Self {
        let graphics_sys = ctx.get::<GraphicsSystem>();
        let device = graphics_sys.device();
        let queue = graphics_sys.queue();

        let params = shader_types::Params { tick: 0 };
        let params_bytes = [0u8; size_of::<shader_types::Params>()];
        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Params buffer"),
            size: params_bytes.len() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let debug_tex = TextureBundle::new(
            device,
            queue,
            "Debug texture",
            wgpu::TextureDescriptor {
                label: Some("debug_tex0"),
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                sample_count: 1,
                size: wgpu::Extent3d {
                    width: GR_W as u32,
                    height: GR_H as u32,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                usage: wgpu::TextureUsages::STORAGE_BINDING
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            },
            &wgpu::TextureViewDescriptor::default(),
            &wgpu::SamplerDescriptor {
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Nearest,
                min_filter: wgpu::FilterMode::Nearest,
                mipmap_filter: wgpu::MipmapFilterMode::Nearest,
                ..Default::default()
            },
            None,
        );

        const GR_W: usize = GRID_WIDTH as usize;
        const GR_H: usize = GRID_HEIGHT as usize;

        // Create Vec<C> where C: CellStruct
        let count = Self::total_nr_cells();
        let mut cells = Vec::with_capacity(count);
        cells.resize_with(count, C::default);
        // Cast the cells into an encase buffer
        let mut encase_cells_buffer = encase::StorageBuffer::new(Vec::<u8>::new());
        encase_cells_buffer.write(&cells).unwrap();

        // Create the CPU edit buffer
        let mut cells_cpu_buffer = Vec::with_capacity(count);
        cells_cpu_buffer.resize_with(count, || Self::create_maybecell(None));
        let cells_cpu_buffer = cells_cpu_buffer.into_boxed_slice();
        // Cast the maybecells into an encase buffer
        let mut encase_maybecells_buffer = encase::StorageBuffer::new(Vec::<u8>::new());
        encase_maybecells_buffer.write(&cells_cpu_buffer).unwrap();

        let world_cells_a = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("world cells a buffer"),
            contents: encase_cells_buffer.as_ref(),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
        });
        let world_cells_b = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("world cells b buffer"),
            contents: encase_cells_buffer.as_ref(),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
        });
        let current_cells = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("current_cells"),
            contents: encase_cells_buffer.as_ref(),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
        });
        let cpu_to_gpu_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("cells cpu->gpu buffer"),
            contents: encase_maybecells_buffer.as_ref(),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
        });
        let cells_desired_ssbo = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("cells desired buffer"),
            contents: encase_cells_buffer.as_ref(),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
        });

        let intents_ssbo = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("intents buffer"),
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
            size: size_of::<shader_types::Intent>() as u64 * (GR_W * GR_H) as u64,
        });
        let winners_ssbo = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("winners buffer"),
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
            size: size_of::<[u32; GR_W * GR_H]>() as u64,
        });
        let accepted_ssbo = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("accepted buffer"),
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
            size: size_of::<[u32; GR_W * GR_H]>() as u64,
        });

        // Remember to update create_simulation_passes with the new count if this changes
        let count = Self::total_nr_cells();
        // Create the CPU edit buffer
        let mut rb_cells_cpu = Vec::with_capacity(count);
        rb_cells_cpu.resize_with(count, RBCell::default);
        let rb_cells_cpu = rb_cells_cpu.into_boxed_slice();
        // Cast the rbcells into an encase buffer
        let mut encase_rbcells_buffer = encase::StorageBuffer::new(Vec::<u8>::new());
        encase_rbcells_buffer.write(&rb_cells_cpu).unwrap();
        // Create the GPU buffer for it
        let rb_cells_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("rb_cells buffer"),
            contents: encase_rbcells_buffer.as_ref(),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
        });

        // start off with no Rigidbodies
        let rbs = Vec::<RB>::new();
        // Cast the rbs into an encase buffer
        let mut encase_rbs_buffer = encase::StorageBuffer::new(Vec::<u8>::new());
        encase_rbs_buffer.write(&rbs).unwrap();
        // Create the GPU buffer for it
        let rbs_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("rbs buffer"),
            contents: encase_rbs_buffer.as_ref(),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
        });

        let count = Self::total_nr_cells();
        let mut metadata = Vec::with_capacity(count);
        metadata.resize_with(count, shader_types::RBWorldMetadata::default);
        // Cast the rbcells into an encase buffer
        let mut encase_metadata_buffer = encase::StorageBuffer::new(Vec::<u8>::new());
        encase_metadata_buffer.write(&metadata).unwrap();
        let rb_metadata_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("rb_metadata buffer"),
            contents: encase_metadata_buffer.as_ref(),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
        });

        let sim_bind_group1_builder = BindGroupBuilder::new()
            // input_cells
            .add_binding(
                0,
                wgpu::ShaderStages::COMPUTE,
                wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
            )
            .add_binding_with_resource(
                1,
                wgpu::ShaderStages::COMPUTE,
                wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                cpu_to_gpu_buffer.as_entire_binding(),
            )
            // current_cells
            .add_binding_with_resource(
                2,
                wgpu::ShaderStages::COMPUTE,
                wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                current_cells.as_entire_binding(),
            )
            // intents
            .add_binding_with_resource(
                3,
                wgpu::ShaderStages::COMPUTE,
                wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                intents_ssbo.as_entire_binding(),
            )
            // winners
            .add_binding_with_resource(
                4,
                wgpu::ShaderStages::COMPUTE,
                wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                winners_ssbo.as_entire_binding(),
            )
            // accepted
            .add_binding_with_resource(
                5,
                wgpu::ShaderStages::COMPUTE,
                wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                accepted_ssbo.as_entire_binding(),
            )
            // next_cells
            .add_binding(
                6,
                wgpu::ShaderStages::COMPUTE,
                wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
            )
            // params
            .add_binding_with_resource(
                7,
                wgpu::ShaderStages::COMPUTE,
                wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                params_buffer.as_entire_binding(),
            )
            // desired_cells
            .add_binding_with_resource(
                8,
                wgpu::ShaderStages::COMPUTE,
                wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                cells_desired_ssbo.as_entire_binding(),
            ) // rb_cells
            .add_binding_with_resource(
                9,
                wgpu::ShaderStages::COMPUTE,
                wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                rb_cells_buffer.as_entire_binding(),
            ) // rbs
            .add_binding_with_resource(
                10,
                wgpu::ShaderStages::COMPUTE,
                wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                rbs_buffer.as_entire_binding(),
            ) // rb_metadata
            .add_binding_with_resource(
                11,
                wgpu::ShaderStages::COMPUTE,
                wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                rb_metadata_buffer.as_entire_binding(),
            );

        let (sim_bgl_a_b, sim_bind_group1_a) = sim_bind_group1_builder
            .clone()
            .add_resource_to_binding(0, world_cells_a.as_entire_binding())
            .add_resource_to_binding(6, world_cells_b.as_entire_binding())
            .build("compute bind group A", device);
        let (_, sim_bind_group1_b) = sim_bind_group1_builder
            .clone()
            .add_resource_to_binding(0, world_cells_b.as_entire_binding())
            .add_resource_to_binding(6, world_cells_a.as_entire_binding())
            .build("compute bind group B", device);

        let (debug_bgl, debug_bind_group) = BindGroupBuilder::new()
            .add_binding_with_resource(
                0,
                wgpu::ShaderStages::COMPUTE,
                wgpu::BindingType::StorageTexture {
                    access: wgpu::StorageTextureAccess::WriteOnly,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    view_dimension: wgpu::TextureViewDimension::D2,
                },
                wgpu::BindingResource::TextureView(debug_tex.view()),
            )
            .build("debug compute bind group", device);
        let debug_bind_group = (debug_bind_group, debug_bgl);

        let display_tex = TextureBundle::new(
            device,
            queue,
            "Simulation display texture",
            wgpu::TextureDescriptor {
                label: Some("display tex"),
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                sample_count: 1,
                size: wgpu::Extent3d {
                    width: GR_W as u32,
                    height: GR_H as u32,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                usage: wgpu::TextureUsages::STORAGE_BINDING
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            },
            &wgpu::TextureViewDescriptor::default(),
            &wgpu::SamplerDescriptor {
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Nearest,
                min_filter: wgpu::FilterMode::Nearest,
                mipmap_filter: wgpu::MipmapFilterMode::Nearest,
                ..Default::default()
            },
            None,
        );
        let materials_ssbo = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("materials buffer"),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
            size: size_of::<M>() as u64 * 10,
        });
        let (sim_bgl2, sim_bind_group2) = BindGroupBuilder::new()
            .add_binding_with_resource(
                0,
                wgpu::ShaderStages::COMPUTE,
                wgpu::BindingType::StorageTexture {
                    access: wgpu::StorageTextureAccess::WriteOnly,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    view_dimension: wgpu::TextureViewDimension::D2,
                },
                wgpu::BindingResource::TextureView(display_tex.view()),
            )
            .add_binding_with_resource(
                1,
                wgpu::ShaderStages::COMPUTE,
                wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                materials_ssbo.as_entire_binding(),
            )
            .build("sim bind group 2", device);
        let sim_bind_group2 = (sim_bind_group2, sim_bgl2);

        drop(graphics_sys);
        let (display_tex_handle, debug_tex_handle) = {
            let mut asset_sys = ctx.get_mut::<AssetSystem>();
            (
                asset_sys.register(display_tex),
                asset_sys.register(debug_tex),
            )
        };

        Self {
            ctx,
            frame: 0,
            tickrate: Duration::from_millis(16 * 2),
            last_tick: Instant::now() + Duration::from_secs_f32(0.5), // small delay before sim starts
            accumulator: Duration::ZERO,

            display_scale: 3.5,

            cells_cpu_buffer,
            cells_dirty_indices: Vec::with_capacity((GRID_WIDTH * GRID_HEIGHT / 2) as usize),
            cpu_to_gpu_buffer,

            user_definitions_shader: None,
            user_cell_process_shader: None,
            user_display_shader: None,

            simulation_passes: vec![],
            compute_pl_layout: None,
            #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
            shader_paths: vec![],

            sim_bgl_a_b,
            sim_bind_group1_a,
            sim_bind_group1_b,
            debug_bind_group,
            sim_bind_group2,
            user_bind_group: None,

            params,
            params_bytes,
            params_buffer,

            display_tex_handle,
            debug_tex_handle,

            material_names: HashMap::default(),
            materials: vec![],
            materials_ssbo,

            physics: SimPhysics::new(50),
            rb_cells_cpu,
            rb_cells_buffer,
            rbs,
            rbs_buffer,
            rapier_rb_to_sim_rb: HashMap::default(),
        }
    }
}
