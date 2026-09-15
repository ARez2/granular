#![feature(trait_alias)]

use encase::{ShaderType, UniformBuffer};
use glam::prelude::*;
use granular_core::{
    filewatcher::{self, FileWatcher},
    graphics::BindGroupBuilder,
    prelude::*,
};
use naga::valid::{Capabilities, ValidationFlags, Validator};
use rustc_hash::FxHashMap as HashMap;
#[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
use std::path::PathBuf;
use std::{borrow::Cow, fmt::Display};
use web_time::{Duration, Instant};
use wgpu::{Buffer, util::DeviceExt};
use wgsl_preprocessor::include_file;

use crate::shader_types::MaybeCell;

pub mod prelude {
    pub use super::UserShaderInput;
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
{
}
/// Needs `#[derive(ShaderType, Clone)]` and a `Default` implementation
pub trait MaterialShaderStruct = 'static
    + Default
    + Clone
    + encase::ShaderType
    + encase::ShaderSize
    + encase::internal::WriteInto;
/// Needs `#[derive(ShaderType, Clone, Copy)]` and a `Default` implementation
pub trait CellStruct = Sized
    + 'static
    + Default
    + Clone
    + Copy
    + encase::ShaderType
    + encase::ShaderSize
    + encase::internal::WriteInto;

#[derive(Debug, Clone)]
pub struct UserShaderInput {
    pub main_shader: wgsl_preprocessor::IncludedFile,
    pub includes: Vec<wgsl_preprocessor::IncludedFile>,
}

struct EncaseStaging<'a>(&'a mut wgpu::QueueWriteBufferView);
impl encase::internal::BufferMut for EncaseStaging<'_> {
    fn capacity(&self) -> usize {
        self.0.len()
    }

    fn write<const N: usize>(&mut self, offset: usize, value: &[u8; N]) {
        self.0.slice(offset..offset + N).copy_from_slice(value);
    }

    fn write_slice(&mut self, offset: usize, value: &[u8]) {
        self.0
            .slice(offset..offset + value.len())
            .copy_from_slice(value);
    }
}

fn validate_wgsl(source: &str) -> Result<(), String> {
    let module =
        naga::front::wgsl::parse_str(source).map_err(|error| error.emit_to_string(source))?;

    Validator::new(ValidationFlags::all(), Capabilities::default())
        .validate(&module)
        .map_err(|error| error.emit_to_string(source))?;

    Ok(())
}

/// A falling sand simulation framework. Call `init_simulation` ASAP to initialize the simulation! Otherwise it will not run!
pub struct Simulation<N: MatName, M: MaterialShaderStruct, C: CellStruct> {
    ctx: GeeseContextHandle<Self>,
    pub frame: u64,
    pub tickrate: Duration,
    last_tick: Instant,
    accumulator: Duration,

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
    compute_pipelines: Vec<(String, wgpu::ComputePipeline)>,
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

    /// Maps the enum values to an index in `materials`
    material_names: HashMap<N, usize>,
    /// Contains the materials, like they would be laid out in GPU memory
    materials: Vec<Option<M>>,
    /// The GPU memory containing the materials
    materials_ssbo: Buffer,
}
impl<N: MatName, M: MaterialShaderStruct, C: CellStruct> Simulation<N, M, C> {
    fn update(&mut self, _: &granular_core::graphics::events::RecordGameRenderingCommands) {
        if self.compute_pipelines.is_empty() {
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

            for (_pl_name, pipeline) in &self.compute_pipelines {
                #[cfg(feature = "trace")]
                profiling::scope!("compute pass");

                #[cfg(feature = "trace")]
                let mut compute_pass = profiler_scope.scoped_compute_pass(_pl_name);
                #[cfg(not(feature = "trace"))]
                let mut compute_pass =
                    context
                        .encoder
                        .begin_compute_pass(&wgpu::ComputePassDescriptor {
                            label: Some("compute pass"),
                            timestamp_writes: None,
                        });
                compute_pass.set_pipeline(pipeline);
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
                compute_pass.dispatch_workgroups(GRID_WIDTH / 8, GRID_WIDTH / 8, 1);
            }
            self.accumulator -= self.tickrate;
            self.frame += 1;
        }

        // Write back any changes to params into the buffer
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

    fn on_render(&mut self, _: &graphics::events::RecordGameRenderingCommands) {
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
        {
            let mut graphics_sys = self.ctx.get_mut::<GraphicsSystem>();
            let context = graphics_sys.render_context();
            context
                .queue
                .write_buffer(&self.materials_ssbo, 0, &materials_buffer.into_inner());

            if !self.cells_dirty_indices.is_empty() {
                let mut staging = context
                    .queue
                    .write_buffer_with(&self.cpu_to_gpu_buffer, 0, self.cells_cpu_buffer.size())
                    .expect("Invalid buffer write");

                encase::StorageBuffer::new(EncaseStaging(&mut staging))
                    .write(self.cells_cpu_buffer.as_ref())
                    .expect("encase serialization failed");

                for idx in &self.cells_dirty_indices {
                    self.cells_cpu_buffer[*idx].is_some = false as i32;
                }
                self.cells_dirty_indices.clear();
            }
        }

        let mut renderer = self.ctx.get_mut::<BatchRenderer>();
        let size = renderer.get_screen_size();
        renderer.draw_quad_with_bottomleft(
            IVec2::new(0, 0),
            size,
            palette::named::WHITE,
            Some(self.display_tex_handle.clone()),
            -10,
        );
        renderer.mark_quad_texture_dirty(self.display_tex_handle.clone());
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

    #[inline(always)]
    fn pos_to_idx(&self, pos: IVec2) -> usize {
        (pos.y * GRID_WIDTH as i32 + pos.x) as usize
    }

    pub fn set_cell(&mut self, pos: IVec2, cell: C) {
        let idx = self.pos_to_idx(pos);
        self.cells_cpu_buffer[idx].inner_cell = cell;
        self.cells_cpu_buffer[idx].is_some = true as i32;
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

        let mut material_constants = String::new();
        for mat_name in N::iter() {
            let mat_idx: u32 = mat_name.into();
            let wgsl_string = format!(
                "const MAT_{}: u32 = {};\n",
                mat_name.to_string().to_uppercase(),
                mat_idx
            );

            material_constants.push_str(&wgsl_string);
        }
        material_constants.push('\n');
        definitions_shader.source.insert_str(0, &material_constants);

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
                #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
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
            include_file!("shaders/compute_to_process.wgsl"),
            vec![
                include_file!("shaders/shared.wgsl"),
                include_file!("shaders/debug_print.wgsl"),
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
                #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
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
            include_file!("shaders/compute_to_display.wgsl"),
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
                #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
                error!("Shader source written to: {}", dumppath.display());
            }
            return;
        }

        let mut compute_shader = wgsl_preprocessor::Preprocessor::new(
            include_file!("shaders/compute_to_z_end.wgsl"),
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

        let graphics_sys = self.ctx.get::<GraphicsSystem>();
        let device = graphics_sys.device();
        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Simulation main compute shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Owned(full_shader_source)),
        });
        self.compute_pipelines = Self::create_compute_pipelines(
            device,
            self.compute_pl_layout.as_ref().unwrap(),
            &shader_module,
        )
    }

    fn create_compute_pipelines(
        device: &wgpu::Device,
        layout: &wgpu::PipelineLayout,
        compute_shader: &wgpu::ShaderModule,
    ) -> Vec<(String, wgpu::ComputePipeline)> {
        vec![
            (
                String::from("prepare compute pass"),
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("prepare compute pipeline"),
                    layout: Some(layout),
                    module: compute_shader,
                    entry_point: Some("prepare"),
                    compilation_options: Default::default(),
                    cache: None,
                }),
            ),
            (
                String::from("propose compute pass"),
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("propose compute pipeline"),
                    layout: Some(layout),
                    module: compute_shader,
                    entry_point: Some("propose"),
                    compilation_options: Default::default(),
                    cache: None,
                }),
            ),
            (
                String::from("resolve compute pass"),
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("resolve compute pipeline"),
                    layout: Some(layout),
                    module: compute_shader,
                    entry_point: Some("resolve"),
                    compilation_options: Default::default(),
                    cache: None,
                }),
            ),
            (
                String::from("commit compute pass"),
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("commit compute pipeline"),
                    layout: Some(layout),
                    module: compute_shader,
                    entry_point: Some("commit"),
                    compilation_options: Default::default(),
                    cache: None,
                }),
            ),
            (
                String::from("display compute pass"),
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("display compute pipeline"),
                    layout: Some(layout),
                    module: compute_shader,
                    entry_point: Some("display"),
                    compilation_options: Default::default(),
                    cache: None,
                }),
            ),
        ]
    }

    fn create_maybecell(cell: Option<C>) -> shader_types::MaybeCell<C> {
        match cell {
            Some(c) => shader_types::MaybeCell {
                inner_cell: c,
                is_some: true as i32,
            },
            None => shader_types::MaybeCell {
                inner_cell: C::default(),
                is_some: false as i32,
            },
        }
    }
}
impl<N: MatName, M: MaterialShaderStruct, C: CellStruct> GeeseSystem for Simulation<N, M, C> {
    const DEPENDENCIES: Dependencies = dependencies()
        .with::<Mut<GraphicsSystem>>()
        .with::<Mut<BatchRenderer>>()
        .with::<Mut<AssetSystem>>()
        .with::<Mut<FileWatcher>>();

    #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
    const EVENT_HANDLERS: EventHandlers<Self> = event_handlers()
        .with(Self::update)
        .with(Self::on_render)
        .with(Self::on_filechange);
    #[cfg(any(target_arch = "wasm32", not(debug_assertions)))]
    const EVENT_HANDLERS: EventHandlers<Self> =
        event_handlers().with(Self::update).with(Self::on_render);

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

        // Heap-Slice erzeugen, ohne ein großes lokales Array aufzubauen.
        // resize_with benötigt für C nur Default, kein Copy oder Clone.
        let count = (GRID_WIDTH * GRID_HEIGHT) as usize;
        let mut cells = Vec::with_capacity(count);
        cells.resize_with(count, C::default);
        // let cells_buffer = cells.into_boxed_slice();

        let mut cells_cpu_buffer = Vec::with_capacity(count);
        cells_cpu_buffer.resize_with(count, || Self::create_maybecell(None));
        let cells_cpu_buffer = cells_cpu_buffer.into_boxed_slice();

        let mut encase_cells_buffer = encase::StorageBuffer::new(Vec::<u8>::new());
        encase_cells_buffer.write(&cells).unwrap();

        let mut encase_maybecells_buffer = encase::StorageBuffer::new(Vec::<u8>::new());
        encase_maybecells_buffer.write(&cells_cpu_buffer).unwrap();

        let cells_read_ssbo = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("cells read buffer"),
            contents: encase_cells_buffer.as_ref(),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
        });
        let cells_write_ssbo = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("cells write buffer"),
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

        let sim_bind_group1_builder = BindGroupBuilder::new()
            // current_cells
            .add_binding(
                0,
                wgpu::ShaderStages::COMPUTE,
                wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
            )
            // intents
            .add_binding_with_resource(
                1,
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
                2,
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
                3,
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
                4,
                wgpu::ShaderStages::COMPUTE,
                wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
            )
            // params
            .add_binding_with_resource(
                5,
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
                6,
                wgpu::ShaderStages::COMPUTE,
                wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                cells_desired_ssbo.as_entire_binding(),
            )
            .add_binding_with_resource(
                7,
                wgpu::ShaderStages::COMPUTE,
                wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                cpu_to_gpu_buffer.as_entire_binding(),
            );

        let (sim_bgl_a_b, sim_bind_group1_a) = sim_bind_group1_builder
            .clone()
            .add_resource_to_binding(0, cells_read_ssbo.as_entire_binding())
            .add_resource_to_binding(4, cells_write_ssbo.as_entire_binding())
            .build("compute bind group A", device);
        let (_, sim_bind_group1_b) = sim_bind_group1_builder
            .clone()
            .add_resource_to_binding(0, cells_write_ssbo.as_entire_binding())
            .add_resource_to_binding(4, cells_read_ssbo.as_entire_binding())
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
        let display_tex_handle = {
            let mut asset_sys = ctx.get_mut::<AssetSystem>();
            asset_sys.register(display_tex)
        };

        Self {
            ctx,
            frame: 0,
            tickrate: Duration::from_millis(16 * 2),
            last_tick: Instant::now() + Duration::from_secs_f32(0.5), // small delay before sim starts
            accumulator: Duration::ZERO,

            cells_cpu_buffer,
            cells_dirty_indices: Vec::with_capacity((GRID_WIDTH * GRID_HEIGHT / 2) as usize),
            cpu_to_gpu_buffer,

            user_definitions_shader: None,
            user_cell_process_shader: None,
            user_display_shader: None,

            compute_pipelines: vec![],
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

            material_names: HashMap::default(),
            materials: vec![],
            materials_ssbo,
        }
    }
}
