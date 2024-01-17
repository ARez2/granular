use std::borrow::Cow;

use bytemuck_derive::{Pod, Zeroable};
use geese::*;
use log::{warn, info};
use wgpu::{Device, Queue, ShaderModuleDescriptor, SurfaceConfiguration, RenderPipeline, ShaderModule, Surface, TextureViewDescriptor, CommandEncoderDescriptor, SurfaceTexture, TextureView, CommandEncoder};
use winit::dpi::PhysicalSize;

use super::{WindowSystem, GraphicsBackend};

pub type FrameData = Option<(SurfaceTexture, TextureView, CommandEncoder)>;
pub type FrameDataMut<'a> = Option<&'a mut (wgpu::SurfaceTexture, wgpu::TextureView, wgpu::CommandEncoder)>;


#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct Vertex {
    _pos: [f32; 2],
    _tex_coord: [f32; 2],
    _index: u32,
}

pub(self) fn vertex(pos: [i8; 2], tc: [i8; 2], index: i8) -> Vertex {
    Vertex {
        _pos: [pos[0] as f32, pos[1] as f32],
        _tex_coord: [tc[0] as f32, tc[1] as f32],
        _index: index as u32,
    }
}

pub(crate) fn quadmesh() -> (Vec<Vertex>, Vec<u16>) {
    (vec![
        // left rectangle
        vertex([-1, -1], [0, 1], 0),
        vertex([-1, 1], [0, 0], 0),
        vertex([0, 1], [1, 0], 0),
        vertex([0, -1], [1, 1], 0),
        // right rectangle
        vertex([0, -1], [0, 1], 1),
        vertex([0, 1], [0, 0], 1),
        vertex([1, 1], [1, 0], 1),
        vertex([1, -1], [1, 1], 1),
    ], vec![
        // Left rectangle
        0, 1, 2, // 1st
        2, 0, 3, // 2nd
        // Right rectangle
        4, 5, 6, // 1st
        6, 4, 7, // 2nd
    ])
}



pub struct GraphicsSystem {
    ctx: GeeseContextHandle<Self>,
    surface_config: SurfaceConfiguration,
    frame_data: FrameData,
    surface: Surface<'static>,
    device: Device,
    queue: Queue
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

    pub fn device(&self) -> &Device {
        &self.device
    }

    pub fn surface_config(&self) -> &SurfaceConfiguration {
        &self.surface_config
    }

    pub fn queue(&mut self) -> &mut Queue {
        &mut self.queue
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

    pub fn frame_data_mut(&mut self) -> FrameDataMut {
        self.frame_data.as_mut()
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
                    required_features: wgpu::Features::TEXTURE_BINDING_ARRAY,
                    // Make sure we use the texture resolution limits from the adapter, so we can support images the size of the swapchain.
                    required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                        .using_resolution(adapter.limits()),
                },
                None,
            )).expect("Failed to create device");

        

        let window = ctx.get::<WindowSystem>();
        let window_size = window.window_handle().inner_size();
        let surface = backend.instance().create_surface(window.window_handle()).unwrap();
        let swapchain_capabilities = surface.get_capabilities(&adapter);
        drop(window);
        let swapchain_format = swapchain_capabilities.formats[0];
        
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: swapchain_format,
            width: window_size.width,
            height: window_size.height,
            // Note: Having PresentMode::Fifo (as in the example) caused a Swapchain acquire texture timeout
            // See: https://github.com/bevyengine/bevy/issues/3606
            present_mode: wgpu::PresentMode::AutoNoVsync,
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
            frame_data: None
        }
    }
}