use std::borrow::Cow;

use geese::*;
use log::warn;
use wgpu::{Device, Queue, ShaderModuleDescriptor, SurfaceConfiguration, RenderPipeline, ShaderModule, Surface, TextureViewDescriptor, CommandEncoderDescriptor, SurfaceTexture, TextureView, CommandEncoder};

mod window_system;
pub use window_system::WindowSystem;

mod graphics_backend;
pub use graphics_backend::GraphicsBackend;
use winit::dpi::PhysicalSize;

pub struct GraphicsSystem {
    ctx: GeeseContextHandle<Self>,
    device: Device,
    queue: Queue,
    surface: Surface<'static>,
    surface_config: SurfaceConfiguration,
    render_pipeline: RenderPipeline,
    vertex_shader: ShaderModule,
    fragment_shader: ShaderModule,
    frame_data: Option<(SurfaceTexture, TextureView, CommandEncoder)>,
}
impl GraphicsSystem {
    pub fn request_redraw(&self) {
        self.ctx.get::<WindowSystem>().window_handle().request_redraw();
    }


    pub fn resize_surface(&mut self, new_size: &PhysicalSize<u32>) {
        self.surface_config.width = new_size.width.max(1);
        self.surface_config.height = new_size.height.max(1);
        self.surface.configure(&self.device, &self.surface_config);
    }

    pub fn begin_frame(&mut self) {
        let frame = self.surface.get_current_texture().expect("Failed to acquire next swapchain texture");
        let view = frame.texture.create_view(&TextureViewDescriptor{..Default::default()});
        let mut encoder = self.device.create_command_encoder(
            &CommandEncoderDescriptor {
                label: Some("Command encoder")
            });
        self.frame_data = Some((frame, view, encoder))
    }

    pub fn render_pass(&mut self) {
        if self.frame_data.is_none() {
            warn!("No frame data present, begin a frame by calling begin_frame()");
            return;
        };
        let (_, view, encoder) = self.frame_data.as_mut().unwrap();
        let mut rpass =
            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::GREEN),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        rpass.set_pipeline(&self.render_pipeline);
        rpass.draw(0..3, 0..1);
    }


    pub fn present_frame(&mut self) {
        if self.frame_data.is_none() {
            warn!("No frame data present, begin a frame by calling begin_frame()");
            return;
        };
        let (frame, _, encoder) = self.frame_data.take().unwrap();
        self.queue.submit(Some(encoder.finish()));
        frame.present();
    }

    pub fn frame_active(&self) -> bool {
        self.frame_data.is_some()
    }
}
impl GeeseSystem for GraphicsSystem {
    const DEPENDENCIES: Dependencies = dependencies()
        .with::<WindowSystem>()
        .with::<GraphicsBackend>();

    
    fn new(ctx: GeeseContextHandle<Self>) -> Self {
        let backend = ctx.get::<GraphicsBackend>();
        let adapter = backend.adapter();

        // Create the logical device and command queue
        let (device, queue) = pollster::block_on(
            adapter.request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::empty(),
                    // Make sure we use the texture resolution limits from the adapter, so we can support images the size of the swapchain.
                    required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                        .using_resolution(adapter.limits()),
                },
                None,
            )).expect("Failed to create device");

        let defs = wgpu::naga::FastHashMap::default();
        let vert_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Vertex shader"),
            source: wgpu::ShaderSource::Glsl {
                shader: Cow::Borrowed(include_str!("../../../shaders/vertex.vert")),
                stage: wgpu::naga::ShaderStage::Vertex,
                defines: defs.clone()
            }
        });
        let frag_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Fragment shader"),
            source: wgpu::ShaderSource::Glsl {
                shader: Cow::Borrowed(include_str!("../../../shaders/fragment.frag")),
                stage: wgpu::naga::ShaderStage::Fragment,
                defines: defs
            }
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let window = ctx.get::<WindowSystem>();
        let window_size = window.window_handle().inner_size();
        let surface = backend.instance().create_surface(window.window_handle()).unwrap();
        let swapchain_capabilities = surface.get_capabilities(&adapter);
        drop(window);
        let swapchain_format = swapchain_capabilities.formats[0];
    
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &vert_shader,
                entry_point: "main",
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &frag_shader,
                entry_point: "main",
                targets: &[Some(swapchain_format.into())],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });
        
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: swapchain_format,
            width: window_size.width,
            height: window_size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: swapchain_capabilities.alpha_modes[0],
            view_formats: vec![],
        };
    
        surface.configure(&device, &config);

        drop(backend);

        Self {
            ctx,
            device,
            queue,
            surface,
            surface_config: config,
            render_pipeline,
            vertex_shader: vert_shader,
            fragment_shader: frag_shader,
            frame_data: None
        }
    }
}