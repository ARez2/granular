use std::num::NonZeroU64;

use geese::{dependencies, GeeseContextHandle, GeeseSystem, Mut};
use log::{info, warn};
use wgpu::{util::DeviceExt, Buffer, BufferUsages, Extent3d, ImageDataLayout, SamplerDescriptor, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages, TextureViewDescriptor};

use crate::{assets::{AssetHandle, ShaderAsset}, AssetSystem, Camera, Simulation, GRID_HEIGHT, GRID_WIDTH};
use super::{graphics_system::Vertex, GraphicsSystem, TextureBundle};


pub struct SimulationRenderer {
    ctx: GeeseContextHandle<Self>,

    vertex_buffer: Buffer,
    bind_group: wgpu::BindGroup,
    render_pipeline: wgpu::RenderPipeline,
    shader_handle: AssetHandle<ShaderAsset>,
    shaderglobals_buffer: Buffer,

    sim_texture: TextureBundle
}
impl SimulationRenderer {
     pub fn render(&mut self) {
        let graphics_sys = self.ctx.get::<GraphicsSystem>();
        let camera = self.ctx.get::<Camera>();
        graphics_sys.queue().write_buffer(&self.shaderglobals_buffer, 0, bytemuck::cast_slice(&[camera.canvas_transform()]));
        let sim = self.ctx.get::<Simulation>();
        let d = sim.get_grid_texture_data();
        graphics_sys.queue().write_texture(self.sim_texture.texture().as_image_copy(), d, self.sim_texture.data_layout(), self.sim_texture.extent());
        drop(graphics_sys);
        drop(camera);
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
}
impl GeeseSystem for SimulationRenderer {
    const DEPENDENCIES: geese::Dependencies = dependencies()
        .with::<Mut<GraphicsSystem>>()
        .with::<Mut<AssetSystem>>()
        .with::<Camera>()
        .with::<Simulation>();
    
    fn new(mut ctx: geese::GeeseContextHandle<Self>) -> Self {
        info!("SimulationRenderer new");
        let mut asset_sys = ctx.get_mut::<AssetSystem>();
        let shader_handle = asset_sys.load::<ShaderAsset>("shaders/sim_renderer.wgsl", true);
        // Drop the mutable reference, from now on we only need it immutably
        drop(asset_sys);

        let graphics_sys = ctx.get::<GraphicsSystem>();
        let device = graphics_sys.device();
        
        let w = graphics_sys.surface_config().width as f32;
        let h = graphics_sys.surface_config().height as f32;
        // Create vertex buffer; array-of-array of position and texture coordinates
        let vertex_data: [[f32; 2]; 3] = [
            // One full-screen triangle
            // See: https://github.com/parasyte/pixels/issues/180
            [0.0 - w, 0.0 - h],
            [3.0 * w, 0.0 - h],
            [0.0 - w, 3.0 * h],
        ];
        let vertex_data_slice = bytemuck::cast_slice(&vertex_data);
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("SimulationRenderer vertex buffer"),
            contents: vertex_data_slice,
            usage: wgpu::BufferUsages::VERTEX,
        });
        let vertex_buffer_layout = wgpu::VertexBufferLayout {
            array_stride: (vertex_data_slice.len() / vertex_data.len()) as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            }],
        };

        let camera = ctx.get::<Camera>();
        let shaderglobals_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("SimulationRenderer Shader globals buffer"),
            contents: bytemuck::cast_slice(&[camera.canvas_transform()]),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST
        });

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
                    resource: shaderglobals_buffer.as_entire_binding(),
                },
            ],
        });

        // Create pipeline
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("SimulationRenderer pipeline layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let asset_sys = ctx.get::<AssetSystem>();
        let base_shader_module = asset_sys.get(&shader_handle).module();
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("SimulationRenderer renderer pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: base_shader_module,
                entry_point: "vs_main",
                buffers: &[vertex_buffer_layout],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: base_shader_module,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: graphics_sys.surface_config().format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None,
        });

        drop(asset_sys);
        drop(graphics_sys);
        drop(camera);

        Self {
            ctx,

            vertex_buffer,
            bind_group,
            render_pipeline,
            shader_handle,
            shaderglobals_buffer,

            sim_texture
        }
    }
}