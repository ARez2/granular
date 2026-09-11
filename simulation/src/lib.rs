#![feature(trait_alias)]

use encase::{DynamicStorageBuffer, UniformBuffer};
use glam::prelude::*;
use granular_core::{
    graphics::{BindGroupBuilder, DynamicTextureAtlas, IntoGpuColor, TextureHandle},
    prelude::*,
};
use rustc_hash::FxHashMap as HashMap;
use std::borrow::Cow;
use std::hash::Hash;
use web_time::{Duration, Instant};
use wgpu::{BindGroup, BindGroupLayout, Buffer, util::DeviceExt};

#[include_wgsl_oil::include_wgsl_oil("../shaders/shared.wgsl")]
pub mod shared_shader {}

#[include_wgsl_oil::include_wgsl_oil("../shaders/cell.wgsl")]
pub mod cell_shader {}

#[include_wgsl_oil::include_wgsl_oil("../shaders/compute.wgsl")]
mod compute_shader {}

#[include_wgsl_oil::include_wgsl_oil("../shaders/material.wgsl")]
pub mod material_shader {}

pub trait MaterialSet = Sized + 'static + Hash + PartialEq + Eq;

pub struct Simulation<M: MaterialSet> {
    ctx: GeeseContextHandle<Self>,
    pub frame: u64,
    pub tickrate: Duration,
    last_tick: Instant,
    accumulator: Duration,

    _cells_read_ssbo: wgpu::Buffer,
    _cells_write_ssbo: wgpu::Buffer,

    compute_pipelines: Vec<(String, wgpu::ComputePipeline)>,
    bind_group_a: wgpu::BindGroup,
    bind_group_b: wgpu::BindGroup,
    debug_bind_group: wgpu::BindGroup,
    display_bind_group: wgpu::BindGroup,

    params: shared_shader::types::Params,
    params_bytes: [u8; size_of::<shared_shader::types::Params>()],
    params_buffer: wgpu::Buffer,

    display_tex_handle: AssetHandle<TextureBundle>,

    /// Maps the enum values to an index in `materials`
    material_names: HashMap<M, usize>,
    material_tex_atlas: DynamicTextureAtlas,
    /// Stores if a new material texture was added and the atlas needs to be rebuilt
    material_atlas_dirty: bool,
    /// Contains the materials, like they would be laid out in GPU memory
    materials: Vec<Option<material_shader::types::Material>>,
    /// The GPU memory containing the materials
    materials_ssbo: Buffer,
    dirty_materials: Vec<usize>,
    materials_bg: (BindGroup, BindGroupLayout),
}
impl<M: MaterialSet> Simulation<M> {
    fn update(&mut self, _: &granular_core::graphics::events::RecordGameRenderingCommands) {
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
                    compute_pass.set_bind_group(0, &self.bind_group_a, &[]);
                } else {
                    compute_pass.set_bind_group(0, &self.bind_group_b, &[]);
                }
                compute_pass.set_bind_group(1, &self.display_bind_group, &[]);
                compute_pass.set_bind_group(2, &self.materials_bg.0, &[]);
                compute_pass.set_bind_group(4, &self.debug_bind_group, &[]);
                compute_pass.dispatch_workgroups(
                    shared_shader::constants::GRID_WIDTH::VALUE / 8,
                    shared_shader::constants::GRID_WIDTH::VALUE / 8,
                    1,
                );
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

        self.last_tick = Instant::now();
        drop(graphics_sys);
    }

    fn on_render(&mut self, _: &graphics::events::RecordGameRenderingCommands) {
        let device = self.ctx.get_mut::<GraphicsSystem>().device().clone();

        if self.material_atlas_dirty {
            let mut atlas_encoder =
                device.create_command_encoder(&wgpu::wgt::CommandEncoderDescriptor {
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

        let default_mat = material_shader::types::Material {
            tex_coords_start: Vec2::ZERO,
            tex_coords_end: Vec2::ZERO,
            color: Vec4::new(1.0, 0.0, 1.0, 1.0),
            density: 0.0,
        };
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
            .collect::<Vec<material_shader::types::Material>>();
        let _ = materials_buffer.write(&mats);
        {
            let mut graphics_sys = self.ctx.get_mut::<GraphicsSystem>();
            let context = graphics_sys.render_context();
            context
                .queue
                .write_buffer(&self.materials_ssbo, 0, &materials_buffer.into_inner());
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

    pub fn add_material(
        &mut self,
        material_name: M,
        mut material_def: material_shader::types::Material,
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
        self.dirty_materials.push(index);
    }
}
impl<M: MaterialSet> GeeseSystem for Simulation<M> {
    const DEPENDENCIES: Dependencies = dependencies()
        .with::<Mut<GraphicsSystem>>()
        .with::<Mut<BatchRenderer>>()
        .with::<Mut<AssetSystem>>();
    const EVENT_HANDLERS: EventHandlers<Self> =
        event_handlers().with(Self::update).with(Self::on_render);

    fn new(mut ctx: GeeseContextHandle<Self>) -> Self {
        let graphics_sys = ctx.get::<GraphicsSystem>();
        let device = graphics_sys.device();
        let queue = graphics_sys.queue();

        let params = shared_shader::types::Params { tick: 0 };
        let params_bytes = [0u8; size_of::<shared_shader::types::Params>()];
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

        const GR_W: usize = shared_shader::constants::GRID_WIDTH::VALUE as usize;
        const GR_H: usize = shared_shader::constants::GRID_HEIGHT::VALUE as usize;

        let mut cells_byte_buffer: Vec<u8> = Vec::new();
        let mut cells_buffer = DynamicStorageBuffer::new(&mut cells_byte_buffer);
        let mut cells = vec![
            cell_shader::types::Cell {
                material: material_shader::constants::MAT_EMPTY::VALUE,
                velocity: Vec2::ZERO,
                _pad: 0.1234,
                color: Vec4::new(0.0, 0.0, 0.0, 1.0)
            };
            GR_W * GR_H
        ];
        let half_w = GR_W / 2;
        let quart_w = GR_W / 4;
        let third_h = GR_H / 3;
        let fith_h = GR_H / 5;
        for y in (third_h - fith_h)..(third_h + fith_h) {
            for x in (half_w - quart_w)..(half_w + quart_w) {
                cells[y * GR_W + x].material = material_shader::constants::MAT_SAND::VALUE;
                cells[y * GR_W + x].color = Vec4::new(1.0, 1.0, 0.0, 1.0);
            }
        }

        for y in 40..47 {
            for x in 0..half_w {
                cells[y * GR_W + x].material = material_shader::constants::MAT_STONE::VALUE;
                cells[y * GR_W + x].color = Vec4::new(0.2, 0.2, 0.2, 1.0);
            }
        }

        for y in (GR_H - 3)..GR_H {
            for x in 0..GR_W {
                cells[y * GR_W + x].material = material_shader::constants::MAT_WATER::VALUE;
                cells[y * GR_W + x].color = Vec4::new(0.0, 0.0, 1.0, 1.0);
            }
        }
        cells_buffer.write(&cells).unwrap();

        let cells_read_ssbo = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("cells read buffer"),
            contents: bytemuck::cast_slice(&cells_byte_buffer),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
        });
        let cells_write_ssbo = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("cells write buffer"),
            contents: bytemuck::cast_slice(&cells_byte_buffer),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
        });
        let cells_desired_ssbo = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("cells desired buffer"),
            contents: bytemuck::cast_slice(&cells_byte_buffer),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
        });

        let intents_ssbo = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("intents buffer"),
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
            size: size_of::<shared_shader::types::Intent>() as u64 * (GR_W * GR_H) as u64,
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

        let compute_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("compute shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(compute_shader::SOURCE)),
        });

        let bind_group_builder = BindGroupBuilder::new()
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
            );

        let (bind_group_layout_a_b, bind_group_a) = bind_group_builder
            .clone()
            .add_resource_to_binding(0, cells_read_ssbo.as_entire_binding())
            .add_resource_to_binding(4, cells_write_ssbo.as_entire_binding())
            .build("compute bind group A", device);
        let (_, bind_group_b) = bind_group_builder
            .clone()
            .add_resource_to_binding(0, cells_write_ssbo.as_entire_binding())
            .add_resource_to_binding(4, cells_read_ssbo.as_entire_binding())
            .build("compute bind group B", device);

        let material_tex_atlas =
            DynamicTextureAtlas::new(device, queue, 2048, 2048, wgpu::FilterMode::Nearest);
        let materials_ssbo = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("materials buffer"),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
            size: size_of::<material_shader::types::Material>() as u64 * 10,
        });
        let (material_bgl, material_bg) = BindGroupBuilder::new()
            .add_binding_with_resource(
                0,
                wgpu::ShaderStages::COMPUTE,
                wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                materials_ssbo.as_entire_binding(),
            )
            .add_binding_with_resource(
                1,
                wgpu::ShaderStages::COMPUTE,
                wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                wgpu::BindingResource::TextureView(material_tex_atlas.view()),
            )
            .add_binding_with_resource(
                2,
                wgpu::ShaderStages::COMPUTE,
                wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                wgpu::BindingResource::Sampler(material_tex_atlas.sampler()),
            )
            .build("Materials bind group", device);
        let materials_bg = (material_bg, material_bgl);

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
        let (display_bgl, display_bind_group) = BindGroupBuilder::new()
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
            .build("display compute bind group", device);

        let compute_pl_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("compute pipeline layout"),
            bind_group_layouts: &[
                Some(&bind_group_layout_a_b),
                Some(&display_bgl),
                Some(&materials_bg.1),
                None,
                Some(&debug_bgl),
            ],
            immediate_size: 0,
        });

        let compute_pipelines = vec![
            (
                String::from("prepare compute pass"),
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("prepare compute pipeline"),
                    layout: Some(&compute_pl_layout),
                    module: &compute_shader,
                    entry_point: Some("prepare"),
                    compilation_options: Default::default(),
                    cache: None,
                }),
            ),
            (
                String::from("propose compute pass"),
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("propose compute pipeline"),
                    layout: Some(&compute_pl_layout),
                    module: &compute_shader,
                    entry_point: Some("propose"),
                    compilation_options: Default::default(),
                    cache: None,
                }),
            ),
            (
                String::from("resolve compute pass"),
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("resolve compute pipeline"),
                    layout: Some(&compute_pl_layout),
                    module: &compute_shader,
                    entry_point: Some("resolve"),
                    compilation_options: Default::default(),
                    cache: None,
                }),
            ),
            (
                String::from("commit compute pass"),
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("commit compute pipeline"),
                    layout: Some(&compute_pl_layout),
                    module: &compute_shader,
                    entry_point: Some("commit"),
                    compilation_options: Default::default(),
                    cache: None,
                }),
            ),
            (
                String::from("display compute pass"),
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("display compute pipeline"),
                    layout: Some(&compute_pl_layout),
                    module: &compute_shader,
                    entry_point: Some("display"),
                    compilation_options: Default::default(),
                    cache: None,
                }),
            ),
        ];

        drop(graphics_sys);
        let display_tex_handle = {
            let mut asset_sys = ctx.get_mut::<AssetSystem>();
            asset_sys.register(display_tex)
        };

        Self {
            ctx,
            frame: 0,
            tickrate: Duration::from_millis(16),
            last_tick: Instant::now() - Duration::from_secs(1),
            accumulator: Duration::ZERO,

            _cells_read_ssbo: cells_read_ssbo,
            _cells_write_ssbo: cells_write_ssbo,

            compute_pipelines,
            bind_group_a,
            bind_group_b,
            debug_bind_group,
            display_bind_group,

            params,
            params_bytes,
            params_buffer,

            display_tex_handle,

            material_names: HashMap::default(),
            material_tex_atlas,
            material_atlas_dirty: false,
            materials: vec![],
            materials_ssbo,
            dirty_materials: vec![],
            materials_bg,
        }
    }
}
