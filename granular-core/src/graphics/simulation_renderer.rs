use std::num::NonZeroU64;

use geese::{dependencies, event_handlers, EventHandlers, GeeseContextHandle, GeeseSystem, Mut};
use log::warn;
use wgpu::{util::DeviceExt, BindGroup, BindGroupLayout, Buffer, ColorTargetState, Device, Extent3d, ImageDataLayout, RenderPipeline, SamplerDescriptor, ShaderModule, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages, TextureViewDescriptor};
use winit::dpi::PhysicalSize;

use crate::{assets::{AssetHandle, ShaderAsset}, AssetSystem, Camera, Simulation, GRID_HEIGHT, GRID_WIDTH};
use super::{GraphicsSystem, TextureBundle};


pub struct SimulationRenderer {
    ctx: GeeseContextHandle<Self>,

    vertex_buffer: Buffer,
    bind_group: BindGroup,
    bind_group_layout: BindGroupLayout,
    render_pipeline: wgpu::RenderPipeline,
    color_target_state: Option<ColorTargetState>,
    vertex_size: u64,
    shader_handle: AssetHandle<ShaderAsset>,

    sim_texture: TextureBundle
}
impl SimulationRenderer {
     pub fn render(&mut self) {
        let graphics_sys = self.ctx.get::<GraphicsSystem>();
        let sim = self.ctx.get::<Simulation>();
        let d = sim.get_grid_texture_data();
        graphics_sys.queue().write_texture(self.sim_texture.texture().as_image_copy(), d, self.sim_texture.data_layout(), self.sim_texture.extent());
        drop(graphics_sys);
        drop(sim);

        let mut graphics_sys = self.ctx.get_mut::<GraphicsSystem>();
        let framedata = graphics_sys.frame_data_mut();
        if framedata.is_none() {
            warn!("No frame data present, call begin_frame first!");
            return;
        };
        let framedata = framedata.unwrap();

        let mut rpass = framedata.2.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("SimulationRenderer render pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &framedata.1,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None
        });
        rpass.set_pipeline(&self.render_pipeline);
        rpass.set_bind_group(0, &self.bind_group, &[]);
        rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        rpass.draw(0..3, 0..1);
    }


    fn get_vertex_data(window_size: (u32, u32)) -> [[f32; 2]; 3] {
        let w = window_size.0 as f32;
        let h = window_size.1 as f32;
        // Create vertex buffer; array-of-array of position and texture coordinates
        [
            // One full-screen triangle
            // See: https://github.com/parasyte/pixels/issues/180
            [0.0 - w, 0.0 - h],
            [3.0 * w, 0.0 - h],
            [0.0 - w, 3.0 * h],
        ]
    }

    pub(super) fn resize(&mut self, new_size: PhysicalSize<u32>) {
        let vertex_data = Self::get_vertex_data((new_size.width, new_size.height));
        let graphics_sys = self.ctx.get::<GraphicsSystem>();
        graphics_sys.queue().write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertex_data));
    }


    /// Reloads parts of the renderer depending on what asset changed
    fn on_assetchange(&mut self, event: &crate::assets::events::AssetReload) {
        if event.asset_id == **self.shader_handle.id() {
            self.reload_render_pipeline();
        }
    }


    /// Helper function to set up a new render pipeline using the same shaders
    fn reload_render_pipeline(&mut self) {
        let graphics_sys = self.ctx.get::<GraphicsSystem>();
        let asset_sys = self.ctx.get::<AssetSystem>();
        let base_shader_module = asset_sys.get(&self.shader_handle).module();
        self.render_pipeline = Self::create_render_pipeline(graphics_sys.device(), &self.bind_group_layout, &base_shader_module, &self.color_target_state, self.vertex_size);
    }


    /// Helper function for creating a new render pipeline
    fn create_render_pipeline(
        device: &Device,
        bind_group_layout: &BindGroupLayout,
        shader: &ShaderModule,
        color_state: &Option<ColorTargetState>,
        vertex_size: u64
    ) -> RenderPipeline {
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("SimulationRenderer render pipeline layout"),
            bind_group_layouts: &[bind_group_layout],
            push_constant_ranges: &[],
        });
        
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("SimulationRenderer renderer pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: vertex_size as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x2,
                        offset: 0,
                        shader_location: 0,
                    }],
                }],
                compilation_options: Default::default()
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: "fs_main",
                targets: &[color_state.clone()],
                compilation_options: Default::default()
            }),
            multiview: None,
            cache: None,
        })
    }
}
impl GeeseSystem for SimulationRenderer {
    const DEPENDENCIES: geese::Dependencies = dependencies()
        .with::<Mut<GraphicsSystem>>()
        .with::<Mut<AssetSystem>>()
        .with::<Camera>()
        .with::<Simulation>();

    const EVENT_HANDLERS: EventHandlers<Self> = event_handlers()
        .with(Self::on_assetchange);
    
    fn new(mut ctx: geese::GeeseContextHandle<Self>) -> Self {
        let mut asset_sys = ctx.get_mut::<AssetSystem>();
        let shader_handle = asset_sys.load::<ShaderAsset>("shaders/sim_renderer.wgsl", true);
        // Drop the mutable reference, from now on we only need it immutably
        drop(asset_sys);

        let graphics_sys = ctx.get::<GraphicsSystem>();
        let vertex_data = Self::get_vertex_data((graphics_sys.surface_config().width, graphics_sys.surface_config().height));
        let device = graphics_sys.device();
        let vertex_data_slice = bytemuck::cast_slice(&vertex_data);
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("SimulationRenderer vertex buffer"),
            contents: vertex_data_slice,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        let vertex_size = (vertex_data_slice.len() / vertex_data.len()) as u64;

        let tex_extent = Extent3d {width: GRID_WIDTH as u32, height: GRID_HEIGHT as u32, depth_or_array_layers: 1};
        let sim_tex_data = [0u8; GRID_WIDTH * GRID_HEIGHT * 4];
        let sim_texture = TextureBundle::new(
            device,
            graphics_sys.queue(),
            "SimulationRenderer sim_texture bundle",
            tex_extent,
            TextureDescriptor {
                label: Some("SimulationRenderer sim_texture descriptor"),
                size: tex_extent,
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba8UnormSrgb,
                usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
                view_formats: &[]
            },
            &TextureViewDescriptor::default(),
            &SamplerDescriptor {
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Nearest,
                min_filter: wgpu::FilterMode::Nearest,
                mipmap_filter: wgpu::FilterMode::Nearest,
                ..Default::default()
            },
            &sim_tex_data,
            ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * tex_extent.width),
                rows_per_image: Some(tex_extent.height),
            }
        );
    
        // Create bind group
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SimulationRenderer bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: Some(NonZeroU64::new(64).unwrap()),
                    },
                    count: None,
                },
            ],
        });
        let camera = ctx.get::<Camera>();
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SimulationRenderer bind group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(sim_texture.view()),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sim_texture.sampler()),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: camera.canvas_transform_buffer().as_entire_binding(),
                },
            ],
        });

        // Create pipeline
        let asset_sys = ctx.get::<AssetSystem>();
        let base_shader_module = asset_sys.get(&shader_handle).module();
        let color_target_state = Some(wgpu::ColorTargetState {
            format: graphics_sys.surface_config().format,
            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            write_mask: wgpu::ColorWrites::ALL,
        });
        let render_pipeline = Self::create_render_pipeline(device, &bind_group_layout, &base_shader_module, &color_target_state, vertex_size);

        drop(asset_sys);
        drop(graphics_sys);
        drop(camera);

        Self {
            ctx,

            vertex_buffer,
            bind_group,
            bind_group_layout,
            render_pipeline,
            color_target_state,
            vertex_size,
            shader_handle,

            sim_texture
        }
    }
}