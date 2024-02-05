use std::borrow::Cow;

use geese::{GeeseSystem, dependencies, GeeseContextHandle, Mut, EventHandlers, event_handlers};
use log::{warn, info, error};
use wgpu::{BindGroup, BindGroupLayout, Buffer, Color, ColorTargetState, Device, Extent3d, IndexFormat, RenderPipeline, RenderPipelineDescriptor, ShaderModule, ShaderModuleDescriptor};
use wgpu::util::DeviceExt;
use winit::dpi::PhysicalSize;

use crate::assets::{AssetHandle, AssetServer, ShaderAsset, TextureAsset};

use super::graphics_system::{Vertex, VERTEX_SIZE};
use super::{GraphicsSystem, graphics_system::quadmesh};



pub struct Renderer2D {
    ctx: GeeseContextHandle<Self>,
    vertex_buffer: Buffer,
    index_buffer: Buffer,
    bind_group: BindGroup,
    bind_group_layout: BindGroupLayout,
    index_format: IndexFormat,
    num_indices: u32,
    clear_color: Color,
    extents: Extent3d,
    shader_handle: AssetHandle<ShaderAsset>,
    render_pipeline: RenderPipeline,

    background_image: AssetHandle<TextureAsset>,
}
impl Renderer2D {
    pub fn render(&mut self) {
        let mut graphics_sys = self.ctx.get_mut::<GraphicsSystem>();
        graphics_sys.begin_frame();
        let framedata = graphics_sys.frame_data_mut();
        if framedata.is_none() {
            warn!("No frame data present, call begin_frame first!");
            return;
        };
        let framedata = framedata.unwrap();

        let mut rpass = framedata.2.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &framedata.1,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        rpass.set_pipeline(&self.render_pipeline);
        rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        rpass.set_index_buffer(self.index_buffer.slice(..), self.index_format);
        rpass.set_bind_group(0, &self.bind_group, &[]);
        rpass.draw_indexed(0..self.num_indices, 0, 0..1);

        drop(rpass);
        
        graphics_sys.present_frame();
    }


    pub fn request_redraw(&self) {
        let graphics_sys = self.ctx.get::<GraphicsSystem>();
        graphics_sys.request_redraw();
    }


    pub(crate) fn resize(&mut self, new_size: &PhysicalSize<u32>) {
        let mut graphics_sys = self.ctx.get_mut::<GraphicsSystem>();
        graphics_sys.resize_surface(new_size);
    }

    fn on_assetchange(&mut self, event: &crate::assets::events::AssetReload) {
        if event.asset_id == self.shader_handle.id() {
            let graphics_sys = self.ctx.get::<GraphicsSystem>();
            let asset_sys = self.ctx.get::<AssetServer>();
            let shader = asset_sys.get(&self.shader_handle);
            self.render_pipeline = Self::create_render_pipeline(
                graphics_sys.device(),
                &self.bind_group_layout,
                shader.module(),
                Some(graphics_sys.surface_config().format.into()))
        } else if event.asset_id == self.background_image.id() {
            let graphics_sys = self.ctx.get::<GraphicsSystem>();
            let device = graphics_sys.device();
            let asset_sys = self.ctx.get::<AssetServer>();
            let background_tex = asset_sys.get(&self.background_image);
            self.bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(background_tex.view()),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(background_tex.sampler()),
                    }
                ],
                layout: &self.bind_group_layout,
                label: Some("bind group"),
            });
        }
    }


    fn create_render_pipeline(
        device: &Device,
        bind_group_layout: &BindGroupLayout,
        shader: &ShaderModule,
        color_state: Option<ColorTargetState>
    ) -> RenderPipeline {
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("main"),
            bind_group_layouts: &[bind_group_layout],
            push_constant_ranges: &[],
        });

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: "vert_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: VERTEX_SIZE as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Sint32],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: "uniform_main",
                targets: &[color_state],
            }),
            primitive: wgpu::PrimitiveState {
                front_face: wgpu::FrontFace::Ccw,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        })
    }
}

impl GeeseSystem for Renderer2D {
    const DEPENDENCIES: geese::Dependencies = dependencies()
        .with::<Mut<GraphicsSystem>>()
        .with::<Mut<AssetServer>>();

    const EVENT_HANDLERS: EventHandlers<Self> = event_handlers()
        .with(Self::on_assetchange);

    fn new(mut ctx: geese::GeeseContextHandle<Self>) -> Self {
        let mut asset_sys = ctx.get_mut::<AssetServer>();
        let cat_handle = asset_sys.load::<TextureAsset>("assets/cat.jpg", true);
        let base_shader_handle = asset_sys.load::<ShaderAsset>("shaders/base.wgsl", true);
        drop(asset_sys);

        let graphics_sys = ctx.get::<GraphicsSystem>();
        let device = graphics_sys.device();

        let quadmesh = quadmesh();
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&quadmesh.0),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(&quadmesh.1),
            usage: wgpu::BufferUsages::INDEX,
        });
        let num_indices = quadmesh.1.len() as u32;

        let conf = graphics_sys.surface_config();
        let extents = wgpu::Extent3d { width: conf.width, height: conf.height, depth_or_array_layers: 1 };

        let device = graphics_sys.device();
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                }
            ],
        });

        let asset_sys = ctx.get::<AssetServer>();
        let cat_texture = asset_sys.get(&cat_handle);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(cat_texture.view()),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(cat_texture.sampler()),
                }
            ],
            layout: &bind_group_layout,
            label: Some("bind group"),
        });

        let base_shader_module = asset_sys.get(&base_shader_handle);
        let render_pipeline = Self::create_render_pipeline(
            &device,
            &bind_group_layout,
            &base_shader_module.module(),
            Some(graphics_sys.surface_config().format.into())
        );

        drop(graphics_sys);
        drop(asset_sys);

        Self {
            ctx,
            vertex_buffer,
            index_buffer,
            index_format: wgpu::IndexFormat::Uint16,
            num_indices,
            bind_group,
            bind_group_layout,
            shader_handle: base_shader_handle,
            render_pipeline,
            clear_color: Color::BLACK,
            extents,

            background_image: cat_handle
        }
    }
}

