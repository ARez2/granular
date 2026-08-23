#![allow(unused)]

use bytemuck_derive::{Pod, Zeroable};
use glam::{IVec2, Vec2};
use rustc_hash::FxHashMap;
use wgpu::{
    Adapter, CommandEncoder, CommandEncoderDescriptor, CurrentSurfaceTexture, Device, Instance,
    Queue, Surface, SurfaceConfiguration, SurfaceTexture, TextureView, TextureViewDescriptor,
};
#[cfg(feature = "trace")]
use wgpu_profiler::{GpuProfiler, GpuProfilerSettings, GpuTimerQueryResult};
use winit::{
    dpi::PhysicalSize,
    event_loop::{ActiveEventLoop, EventLoopProxy},
};

use super::WindowSystem;
use crate::{
    CustomWinitEvent,
    graphics::{Texture2D, texture::TextureHandle},
    utils::*,
};

pub type FrameData = (
    SurfaceTexture,
    TextureView,
    CommandEncoder,
    std::sync::Arc<winit::window::Window>,
);
pub type FrameDataMut<'a> = Option<&'a mut FrameData>;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub(crate) struct Vertex {
    _pos: IVec2,
    _col: [f32; 4],
    _tex_coord: Vec2,
}
impl Vertex {
    pub fn new(pos: IVec2, color: [f32; 4], tex_coord: Vec2) -> Self {
        Self {
            _pos: pos,
            _col: color,
            _tex_coord: tex_coord,
        }
    }
}
pub const VERTEX_SIZE: usize = std::mem::size_of::<Vertex>();

/// This holds the main information for the GraphicsBackend. It is being sent out as an event after the async initialization
#[derive(Debug)]
pub struct GraphicsState {
    instance: Instance,
    adapter: Adapter,
    surface_config: SurfaceConfiguration,
    frame_data: Option<FrameData>,
    surface: Surface<'static>,
    device: Device,
    queue: Queue,
}

#[allow(clippy::large_enum_variant)]
enum GraphicsSystemState {
    Uninitialized,
    Loading,
    Ready(GraphicsState),
}

pub struct GraphicsSystem {
    ctx: GeeseContextHandle<Self>,
    next_texture_id: u32,
    texture_storage: FxHashMap<TextureHandle, Box<dyn Texture2D>>,
    state: GraphicsSystemState,
}
#[profiling::all_functions]
impl GraphicsSystem {
    pub fn init(&mut self, event_loop: &ActiveEventLoop, proxy: EventLoopProxy<CustomWinitEvent>) {
        if !matches!(self.state, GraphicsSystemState::Uninitialized) {
            return;
        }
        self.state = GraphicsSystemState::Loading;

        let window_sys = self.ctx.get::<WindowSystem>();
        let window = window_sys.window_handle();
        drop(window_sys);
        let display_handle = event_loop.owned_display_handle();

        let mut size = window.inner_size();
        size.width = size.width.max(1);
        size.height = size.height.max(1);

        let mut executor = self.ctx.get_mut::<FutureExecutor>();
        executor.spawn_oneshot(async move {
            let mut inst_desc = wgpu::InstanceDescriptor::new_with_display_handle_from_env(
                Box::new(display_handle),
            );
            inst_desc.flags = wgpu::InstanceFlags::advanced_debugging();

            let instance = wgpu::Instance::new(inst_desc);
            let surface = instance.create_surface(window.clone()).unwrap();
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::default(),
                    // Request an adapter which can render to our surface
                    compatible_surface: Some(&surface),
                    ..Default::default()
                })
                .await
                .expect("Failed to find an appropriate adapter");

            let mut features = wgpu::Features::empty();
            #[cfg(feature = "trace")]
            let features = features | GpuProfiler::ALL_WGPU_TIMER_FEATURES;

            // Create the logical device and command queue
            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: Some("wgpu device"),
                    required_features: features,
                    // Make sure we use the texture resolution limits from the adapter,
                    // so we can support images the size of the swapchain.
                    required_limits: adapter.limits(),
                    experimental_features: wgpu::ExperimentalFeatures::disabled(),
                    memory_hints: wgpu::MemoryHints::MemoryUsage,
                    trace: wgpu::Trace::Off,
                })
                .await
                .expect("Failed to create device");

            let mut surface_config = surface
                .get_default_config(&adapter, size.width, size.height)
                .unwrap();
            surface_config.present_mode = wgpu::PresentMode::AutoNoVsync;

            let _ = proxy.send_event(CustomWinitEvent::GraphicsSystemInitialized(GraphicsState {
                instance,
                adapter,
                surface_config,
                frame_data: None,
                surface,
                device,
                queue,
            }));
        });
    }

    pub fn initialize_callback(&mut self, state: GraphicsState) {
        self.state = GraphicsSystemState::Ready(state);

        let window_sys = self.ctx.get::<WindowSystem>();
        let window_size = window_sys.window_handle().inner_size();
        drop(window_sys);
        // winit might have updated the window size while we were
        // creating the surface asynchronously, so resize the surface.
        self.resize_surface(window_size);
    }

    pub fn request_redraw(&self) {
        self.ctx
            .get::<WindowSystem>()
            .window_handle()
            .request_redraw();
    }

    pub fn resize_surface(&mut self, new_size: PhysicalSize<u32>) {
        let GraphicsSystemState::Ready(state) = &mut self.state else {
            return;
        };

        state.surface_config.width = new_size.width.max(1);
        state.surface_config.height = new_size.height.max(1);
        state
            .surface
            .configure(&state.device, &state.surface_config);
    }

    pub fn begin_frame(&mut self) {
        self.device().poll(wgpu::wgt::PollType::Poll);
        let GraphicsSystemState::Ready(state) = &mut self.state else {
            return;
        };

        let window = self.ctx.get::<WindowSystem>().window_handle();

        let frame = match state.surface.get_current_texture() {
            CurrentSurfaceTexture::Success(frame) => frame,
            CurrentSurfaceTexture::Timeout | CurrentSurfaceTexture::Occluded => {
                // Try again later
                window.request_redraw();
                return;
            }
            CurrentSurfaceTexture::Suboptimal(texture) => {
                drop(texture);

                state
                    .surface
                    .configure(&state.device, &state.surface_config);
                window.request_redraw();
                return;
            }
            CurrentSurfaceTexture::Outdated => {
                state
                    .surface
                    .configure(&state.device, &state.surface_config);
                window.request_redraw();
                return;
            }
            CurrentSurfaceTexture::Validation => {
                unreachable!("No error scope registered, so validation errors will panic")
            }
            CurrentSurfaceTexture::Lost => {
                state.surface = state.instance.create_surface(window.clone()).unwrap();
                state
                    .surface
                    .configure(&state.device, &state.surface_config);
                window.request_redraw();
                return;
            }
        };
        let view = frame.texture.create_view(&TextureViewDescriptor {
            ..Default::default()
        });
        let encoder = state
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Command encoder"),
            });
        state.frame_data = Some((frame, view, encoder, window))
    }

    pub fn device(&self) -> &Device {
        if let GraphicsSystemState::Ready(state) = &self.state {
            return &state.device;
        }
        panic!("GraphicsSystem is not ready!");
    }

    pub fn surface_config(&self) -> &SurfaceConfiguration {
        if let GraphicsSystemState::Ready(state) = &self.state {
            return &state.surface_config;
        }
        panic!("GraphicsSystem is not ready!");
    }

    pub fn queue(&self) -> &Queue {
        if let GraphicsSystemState::Ready(state) = &self.state {
            return &state.queue;
        }
        panic!("GraphicsSystem is not ready!");
    }

    pub fn queue_mut(&mut self) -> &mut Queue {
        if let GraphicsSystemState::Ready(state) = &mut self.state {
            return &mut state.queue;
        }
        panic!("GraphicsSystem is not ready!");
    }

    pub fn present_frame(&mut self) {
        let GraphicsSystemState::Ready(state) = &mut self.state else {
            return;
        };

        if state.frame_data.is_none() {
            warn!("No frame data present, begin a frame by calling begin_frame()");
            return;
        };
        let (frame, _, encoder, window) = state.frame_data.take().unwrap();
        state.queue.submit(Some(encoder.finish()));
        window.pre_present_notify();
        state.queue.present(frame);
    }

    pub fn frame_data_mut(&mut self) -> FrameDataMut<'_> {
        if let GraphicsSystemState::Ready(state) = &mut self.state {
            return state.frame_data.as_mut();
        }
        panic!("GraphicsSystem is not ready!");
    }

    fn get_next_texture_id(&mut self) -> u32 {
        let id = self.next_texture_id;
        self.next_texture_id += 1;
        id
    }

    /// Stores a new texture in the GraphicsSystem and returns a handle to it, which can be used to retrieve the texture
    pub fn create_texture(&mut self, texture: Box<dyn Texture2D>) -> TextureHandle {
        let handle = TextureHandle::new(self.get_next_texture_id(), 0);
        self.texture_storage.insert(handle.clone(), texture);
        handle
    }

    /// Uses the handle to get a reference to the texture
    pub fn get_texture(&self, handle: &TextureHandle) -> Option<&dyn Texture2D> {
        self.texture_storage.get(handle).map(|v| &**v)
    }

    /// Uses the handle to get a mutable reference to the texture
    pub fn get_texture_mut(&mut self, handle: &TextureHandle) -> Option<&mut Box<dyn Texture2D>> {
        self.texture_storage.get_mut(handle)
    }
}
#[profiling::all_functions]
impl GeeseSystem for GraphicsSystem {
    const DEPENDENCIES: Dependencies = dependencies()
        .with::<Mut<FutureExecutor>>()
        .with::<WindowSystem>();

    fn new(mut ctx: GeeseContextHandle<Self>) -> Self {
        Self {
            ctx,
            next_texture_id: 0,
            texture_storage: FxHashMap::default(),
            state: GraphicsSystemState::Uninitialized,
        }
    }
}
