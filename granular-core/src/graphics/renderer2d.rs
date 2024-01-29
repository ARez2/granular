use std::borrow::Cow;
use std::num::{NonZeroU64, NonZeroU32};

use geese::{GeeseSystem, dependencies, GeeseContextHandle, Mut, EventHandlers, event_handlers};
use log::{warn, info, error};
use wgpu::{RenderPipeline, Buffer, BindGroup, Color, ShaderModuleDescriptor, Device, Texture, TextureView, Extent3d, IndexFormat};
use wgpu::util::DeviceExt;
use ultraviolet::Mat4;
use winit::dpi::PhysicalSize;

use super::graphics_system::Vertex;
use super::{GraphicsSystem, graphics_system::quadmesh};



pub struct Renderer2D {
    ctx: GeeseContextHandle<Self>,
    vertex_buffer: Buffer,
    index_buffer: Buffer,
    bind_group: BindGroup,
    index_format: IndexFormat,
    num_indices: u32,
    clear_color: Color,
    extents: Extent3d,
    render_pipeline: RenderPipeline,
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

    fn on_filechange(&mut self, event: &crate::filewatcher::events::FilesChanged) {
        event.paths.iter().for_each(|p| {
            if p.ends_with("base.wgsl") {
                info!("Shader changes! Reload GraphicsSystem");
                self.ctx.raise_event(geese::notify::reset_system::<Self>());
                //self.ctx.raise_event(geese::notify::reset_system::<GraphicsSystem>());
            }
        });
    }
}

impl GeeseSystem for Renderer2D {
    const DEPENDENCIES: geese::Dependencies = dependencies()
        .with::<Mut<GraphicsSystem>>();

    const EVENT_HANDLERS: EventHandlers<Self> = event_handlers()
        .with(Self::on_filechange);

    fn new(mut ctx: geese::GeeseContextHandle<Self>) -> Self {
        let mut graphics_sys = ctx.get_mut::<GraphicsSystem>();
        let device = graphics_sys.device();

        let cur = std::env::current_exe().unwrap();
        let base_directory = cur.parent().unwrap().parent().unwrap().parent().unwrap();

        let shader_dir = base_directory.join("shaders");
        let shader_file = shader_dir.join("base.wgsl");
        let shader_contents = std::fs::read_to_string(shader_file);
        let shader_src = match shader_contents {
            Ok(data) => {data},
            Err(e) => {
                error!("Error while reading shader: {:?}", e);
                String::new()
            }
        };
        let base_shader_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Main WGSL shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(&shader_src)),
        });


        let vertex_size = std::mem::size_of::<Vertex>();
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


        let p = base_directory.join("assets").join("cat.jpg");
        let cat_img = image::open(p).unwrap().to_rgba8();
        let mut cat_size = wgpu::Extent3d::default();
        cat_size.width = cat_img.width();
        cat_size.height = cat_img.height();
        //let cat_data = cat_img.as_flat_samples_u8().unwrap();
        //let cat_data = cat_img.as_();
        
        let cat_texture_descriptor = wgpu::TextureDescriptor {
            size: cat_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            label: None,
            view_formats: &[],
        };
        let red_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("red"),
            view_formats: &[],
            ..cat_texture_descriptor
        });
        let red_texture_view = red_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let queue = graphics_sys.queue();
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &red_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &cat_img,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * cat_size.width),
                rows_per_image: Some(cat_size.height),
            },
            cat_size,
        );

        let device = graphics_sys.device();
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

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

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&red_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                }
            ],
            layout: &bind_group_layout,
            label: Some("bind group"),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("main"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let index_format = wgpu::IndexFormat::Uint16;

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &base_shader_module,
                entry_point: "vert_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: vertex_size as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Sint32],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &base_shader_module,
                entry_point: "uniform_main",
                targets: &[Some(graphics_sys.surface_config().format.into())],
            }),
            primitive: wgpu::PrimitiveState {
                front_face: wgpu::FrontFace::Ccw,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        drop(graphics_sys);

        Self {
            ctx,
            vertex_buffer,
            index_buffer,
            index_format,
            num_indices,
            bind_group,
            render_pipeline,
            clear_color: Color::BLACK,
            extents
        }
    }
}

