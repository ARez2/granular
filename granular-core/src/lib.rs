use std::{borrow::Cow, collections::HashMap, hash::BuildHasherDefault};

use geese::*;
use graphics::{Graphics, WindowSystem};
use log::{debug, info};

use winit::event_loop::EventLoop;

//mod tick;
mod graphics;

mod eventloop_system;
pub use eventloop_system::EventLoopSystem;


pub mod events {
    pub struct Initialized {
        
    }

    pub struct NewFrame {
        pub delta: f32,
    }

    pub struct Tick {

    }
}



pub struct GranularEngine {
    ctx: GeeseContextHandle<Self>,
    close_requested: bool
}
impl GranularEngine {
    pub async fn create_window(&mut self, event_loop: &EventLoop<()>, name: &str, size: winit::dpi::LogicalSize<u32>) {
        // let defs: wgpu::naga::FastHashMap<String, String> = wgpu::naga::FastHashMap::default();
        // let vert_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        //     label: None,
        //     source: wgpu::ShaderSource::Glsl {
        //         shader: Cow::Borrowed(include_str!("../../shaders/vert.glsl")),
        //         stage: wgpu::naga::ShaderStage::Vertex,
        //         defines: defs.clone()
        //     },
        // });
        // let frag_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        //     label: None,
        //     source: wgpu::ShaderSource::Glsl {
        //         shader: Cow::Borrowed(include_str!("../../shaders/frag.glsl")),
        //         stage: wgpu::naga::ShaderStage::Fragment,
        //         defines: defs
        //     },
        // });

        // let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        //     label: None,
        //     bind_group_layouts: &[],
        //     push_constant_ranges: &[],
        // });
    
        // let swapchain_capabilities = surface.get_capabilities(&adapter);
        // let swapchain_format = swapchain_capabilities.formats[0];
    
        // let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        //     label: None,
        //     layout: Some(&pipeline_layout),
        //     vertex: wgpu::VertexState {
        //         module: &vert_shader,
        //         entry_point: "main",
        //         buffers: &[],
        //     },
        //     fragment: Some(wgpu::FragmentState {
        //         module: &frag_shader,
        //         entry_point: "main",
        //         targets: &[Some(swapchain_format.into())],
        //     }),
        //     primitive: wgpu::PrimitiveState::default(),
        //     depth_stencil: None,
        //     multisample: wgpu::MultisampleState::default(),
        //     multiview: None,
        // });
        
        // let mut config = wgpu::SurfaceConfiguration {
        //     usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        //     format: swapchain_format,
        //     width: size.width.max(1),
        //     height: size.height.max(1),
        //     present_mode: wgpu::PresentMode::Fifo,
        //     alpha_mode: swapchain_capabilities.alpha_modes[0],
        //     view_formats: vec![],
        // };
    
        // surface.configure(&device, &config);


    }


    pub fn update(&mut self) {
        self.ctx.raise_event(events::NewFrame {delta: 0.0});
        
    }


    pub fn handle_winit_events(&mut self, event: &winit::event::Event<()>) {
        if let winit::event::Event::WindowEvent {
            window_id: _,
            event,
        } = event
        {
            match event {
                winit::event::WindowEvent::CloseRequested => {
                    self.close_requested = true;
                },
                _ => ()
            }
        };
    }

    pub fn use_window_target(&self, target: &winit::event_loop::EventLoopWindowTarget<()>) {
        if self.close_requested {
            target.exit();
        }
    }
}


impl GeeseSystem for GranularEngine {
    const DEPENDENCIES: Dependencies = dependencies()
        .with::<Graphics>();

    const EVENT_HANDLERS: EventHandlers<Self> = event_handlers()
        .with(Self::handle_winit_events);

    fn new(ctx: GeeseContextHandle<Self>) -> Self {
        ctx.raise_event(events::Initialized {});

        Self {
            ctx,
            close_requested: false
        }
    }
}

