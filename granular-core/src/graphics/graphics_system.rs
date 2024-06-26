#![allow(unused)]

use bytemuck_derive::{Pod, Zeroable};
use geese::*;
use glam::{Vec2, IVec2};
use log::*;
use wgpu::{Device, Queue, SurfaceConfiguration, Surface, TextureViewDescriptor, CommandEncoderDescriptor, SurfaceTexture, TextureView, CommandEncoder};
use winit::dpi::PhysicalSize;

use super::{graphics_backend, GraphicsBackend, WindowSystem};

pub type FrameData = Option<(SurfaceTexture, TextureView, CommandEncoder)>;
pub type FrameDataMut<'a> = Option<&'a mut (wgpu::SurfaceTexture, wgpu::TextureView, wgpu::CommandEncoder)>;


#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub(crate) struct Vertex {
    _pos: IVec2,
    _col: [f32; 4],
    _tex_coord: Vec2,
    _tex_idx: u64,
}
impl Vertex {
    pub fn new(pos: IVec2, color: [f32; 4], tex_coord: Vec2, tex_index: u64) -> Self {
        Self {
            _pos: pos,
            _col: color,
            _tex_coord: tex_coord,
            _tex_idx: tex_index,
        }
    }
}
pub const VERTEX_SIZE: usize = std::mem::size_of::<Vertex>();



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


    pub fn resize_surface(&mut self, new_size: PhysicalSize<u32>) {
        self.surface_config.width = new_size.width.max(1);
        self.surface_config.height = new_size.height.max(1);
        self.surface.configure(&self.device, &self.surface_config);
    }

    pub fn begin_frame(&mut self) {
        let frame = self.surface.get_current_texture().expect("Failed to acquire next swapchain texture");
        let view = frame.texture.create_view(&TextureViewDescriptor{..Default::default()});
        let encoder = self.device.create_command_encoder(
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

    pub fn queue(&self) -> &Queue {
        &self.queue
    }

    pub fn queue_mut(&mut self) -> &mut Queue {
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
        .with::<Mut<GraphicsBackend>>();

    fn new(mut ctx: GeeseContextHandle<Self>) -> Self {
        let surface;
        let window_size;
        {
            let immut_backend = ctx.get::<GraphicsBackend>();
            let window = ctx.get::<WindowSystem>();
            window_size = window.window_handle().inner_size();
            surface = immut_backend.instance().create_surface(window.window_handle()).unwrap();
        }
        {
            let mut mut_backend = ctx.get_mut::<GraphicsBackend>();
            let adapter = pollster::block_on(mut_backend.instance().request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })).expect("Could not create an adapter!");
            mut_backend.set_adapter(adapter);
        }

        let backend = ctx.get::<GraphicsBackend>();
        let adapter = backend.adapter();
        // Create the logical device and command queue
        let (device, queue) = pollster::block_on(
            adapter.request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::TEXTURE_BINDING_ARRAY | wgpu::Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING,
                    // Make sure we use the texture resolution limits from the adapter, so we can support images the size of the swapchain.
                    required_limits: adapter.limits(),
                },
                None,
            )).expect("Failed to create device");

        let swapchain_capabilities = surface.get_capabilities(adapter);
        let swapchain_format = swapchain_capabilities.formats.iter()
            .find(|format| {
                format.is_srgb()
            })
            .unwrap_or(&wgpu::TextureFormat::Bgra8UnormSrgb);
        debug!("Swapchain format: {:?}", swapchain_format);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: *swapchain_format,
            width: window_size.width,
            height: window_size.height,
            // Note: Having PresentMode::Fifo (as in the example) caused a Swapchain acquire texture timeout
            // See: https://github.com/bevyengine/bevy/issues/3606
            present_mode: wgpu::PresentMode::AutoNoVsync,
            alpha_mode: swapchain_capabilities.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2
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