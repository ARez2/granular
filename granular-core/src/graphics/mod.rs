use std::borrow::Cow;

use geese::*;
use wgpu::{Device, Queue, ShaderModuleDescriptor, SurfaceConfiguration, RenderPipeline, ShaderModule, Surface};

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
    fragment_shader: ShaderModule
}
impl GraphicsSystem {
    pub fn update_config(&mut self) {
        // self.surface_config = something;
        //self.ctx.get::<GraphicsBackend>().surface().configure(&self.device, &self.surface_config);
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
            fragment_shader: frag_shader
        }
    }
}