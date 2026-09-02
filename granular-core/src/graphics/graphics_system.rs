#![allow(unused)]

use std::sync::Arc;
#[cfg(feature = "trace")]
use std::sync::Mutex;

use anyhow::bail;
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
    window::Window,
};

use super::WindowSystem;
use crate::{
    AssetSystem, CustomWinitEvent,
    graphics::{Texture2D, TextureBundle},
    utils::*,
};

pub struct RenderContext {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub frame: SurfaceTexture,
    pub view: TextureView,
    pub encoder: CommandEncoder,
    #[cfg(feature = "trace")]
    pub profiler: Arc<Mutex<GpuProfiler>>,
}

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
pub struct GraphicsState {
    instance: Instance,
    adapter: Adapter,
    surface_config: SurfaceConfiguration,
    surface: Surface<'static>,
    device: Device,
    queue: Queue,

    #[cfg(feature = "trace")]
    pub profiler: Arc<Mutex<GpuProfiler>>,
    #[cfg(feature = "trace")]
    latest_profiler_results: Option<Vec<GpuTimerQueryResult>>,
}

#[allow(clippy::large_enum_variant)]
enum GraphicsSystemState {
    Uninitialized,
    Loading,
    Ready(GraphicsState),
}

pub struct GraphicsSystem {
    ctx: GeeseContextHandle<Self>,
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

            debug!("{:?}", surface.get_capabilities(&adapter).formats);

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
            let f = Self::calculate_surface_view_format(&surface_config.format);
            surface_config.view_formats = vec![f];

            #[cfg(feature = "trace")]
            let profiler = GpuProfiler::new_with_tracy_client(
                GpuProfilerSettings::default(),
                adapter.get_info().backend,
                &device,
                &queue,
            )
            .unwrap_or_else(|err| match err {
                wgpu_profiler::CreationError::TracyClientNotRunning
                | wgpu_profiler::CreationError::TracyGpuContextCreationError(_) => {
                    println!("Failed to connect to Tracy. Continuing without Tracy integration.");
                    GpuProfiler::new(&device, GpuProfilerSettings::default())
                        .expect("Failed to create profiler")
                }
                _ => {
                    panic!("Failed to create profiler: {err}");
                }
            });

            let _ = proxy.send_event(CustomWinitEvent::GraphicsSystemInitialized(GraphicsState {
                instance,
                adapter,
                surface_config,
                surface,
                device,
                queue,
                #[cfg(feature = "trace")]
                profiler: Arc::new(Mutex::new(profiler)),
                #[cfg(feature = "trace")]
                latest_profiler_results: Default::default(),
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

        {
            let dev = self.device().clone();
            let dev2 = self.device().clone();
            let q = self.queue().clone();
            let mut asset_sys = self.ctx.get_mut::<AssetSystem>();
            // the generic here is technically optional but its clearer this way
            asset_sys.add_loader::<wgpu::ShaderModule>(move |bytes, _settings| {
                Ok(dev.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: None,
                    source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Owned(String::from_utf8(
                        bytes,
                    )?)),
                }))
            });

            asset_sys.add_loader::<TextureBundle>(move |bytes, settings| {
                Ok(TextureBundle::new(
                    &dev2,
                    &q,
                    &settings.name,
                    wgpu::TextureDescriptor {
                        label: Some(&format!("{} descriptor", settings.name)),
                        size: settings.size,
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: settings.format,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING
                            | wgpu::TextureUsages::COPY_DST
                            | wgpu::TextureUsages::COPY_SRC,
                        view_formats: &[],
                    },
                    &wgpu::TextureViewDescriptor::default(),
                    &wgpu::SamplerDescriptor {
                        address_mode_u: wgpu::AddressMode::ClampToEdge,
                        address_mode_v: wgpu::AddressMode::ClampToEdge,
                        address_mode_w: wgpu::AddressMode::ClampToEdge,
                        mag_filter: settings.filtering,
                        min_filter: wgpu::FilterMode::Nearest,
                        mipmap_filter: match settings.filtering {
                            wgpu::FilterMode::Linear => wgpu::MipmapFilterMode::Linear,
                            wgpu::FilterMode::Nearest => wgpu::MipmapFilterMode::Nearest,
                        },
                        ..Default::default()
                    },
                    &bytes,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(
                            crate::graphics::bytes_per_pixel(settings.format).unwrap_or(4)
                                * settings.size.width,
                        ),
                        rows_per_image: Some(settings.size.height),
                    },
                ))
            });
        }
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

        debug!("resize_surface {:?}", new_size);

        #[cfg(target_arch = "wasm32")]
        {
            let mut canvas = crate::graphics::get_canvas();
            canvas.set_width(new_size.width.max(1));
            canvas.set_height(new_size.height.max(1));
            info!("canvas size: {}x{}", canvas.width(), canvas.height());
            info!(
                "canvas client size: {}x{}",
                canvas.client_width(),
                canvas.client_height()
            );
        }
        state.surface_config.width = new_size.width.max(1);
        state.surface_config.height = new_size.height.max(1);
        state
            .surface
            .configure(&state.device, &state.surface_config);
    }

    pub fn begin_frame(&mut self) -> anyhow::Result<RenderContext> {
        self.device().poll(wgpu::wgt::PollType::Poll);
        let GraphicsSystemState::Ready(state) = &mut self.state else {
            bail!("GraphicsSystem is not ready!");
        };

        let window = self.ctx.get::<WindowSystem>().window_handle();

        let frame = match state.surface.get_current_texture() {
            CurrentSurfaceTexture::Success(frame) => frame,
            CurrentSurfaceTexture::Timeout | CurrentSurfaceTexture::Occluded => {
                // Try again later
                window.request_redraw();
                bail!("Surface got a timeout or is occluded. Try again later.");
            }
            CurrentSurfaceTexture::Suboptimal(texture) => {
                drop(texture);

                state
                    .surface
                    .configure(&state.device, &state.surface_config);
                window.request_redraw();
                bail!("Surface isnt optimal. Try again next frame.");
            }
            CurrentSurfaceTexture::Outdated => {
                state
                    .surface
                    .configure(&state.device, &state.surface_config);
                window.request_redraw();
                bail!("The surface is outdated. Try again next frame.");
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
                bail!("The surface has been lost. Try again next frame.");
            }
        };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(Self::calculate_surface_view_format(
                &state.surface_config.format,
            )),
            ..Default::default()
        });
        let encoder = state
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Command encoder"),
            });
        return Ok(RenderContext {
            device: state.device.clone(),
            queue: state.queue.clone(),
            frame,
            view,
            encoder,
            #[cfg(feature = "trace")]
            profiler: state.profiler.clone(),
        });
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

    fn calculate_surface_view_format(surface_format: &wgpu::TextureFormat) -> wgpu::TextureFormat {
        surface_format.add_srgb_suffix()
    }

    /// This is the basically the format we want to display. It gets put into the `view_formats` of the surface.
    /// It causes the surface to have one format as "main" format but then get displayed in this format here.
    /// This allows us to keep using linear values in the shaders and then let wgpu handle gamma correction
    /// on platforms where the default surface format is not gamma corrected (like WASM).
    pub fn get_surface_view_format(&self) -> wgpu::TextureFormat {
        if let GraphicsSystemState::Ready(state) = &self.state {
            return Self::calculate_surface_view_format(&state.surface_config.format);
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

    pub fn present_frame(&mut self, mut context: RenderContext) {
        let GraphicsSystemState::Ready(state) = &mut self.state else {
            return;
        };

        #[cfg(feature = "trace")]
        let mut prof_lock = state
            .profiler
            .lock()
            .expect("Nothing should lock the mutex now");
        #[cfg(feature = "trace")]
        prof_lock.resolve_queries(&mut context.encoder);

        {
            profiling::scope!("wgpu queue submit");
            state.queue.submit(Some(context.encoder.finish()));
        }
        {
            profiling::scope!("wgpu queue present");
            self.ctx
                .get::<WindowSystem>()
                .window_handle()
                .pre_present_notify();
            state.queue.present(context.frame);
        }

        #[cfg(feature = "trace")]
        {
            // Signal to the profiler that the frame is finished.
            prof_lock.end_frame().unwrap();
            // Query for oldest finished frame (this is almost certainly not the one we just submitted!) and display results in the command line.
            state.latest_profiler_results =
                prof_lock.process_finished_frame(state.queue.get_timestamp_period());
        }
    }
}
#[profiling::all_functions]
impl GeeseSystem for GraphicsSystem {
    const DEPENDENCIES: Dependencies = dependencies()
        .with::<Mut<FutureExecutor>>()
        .with::<WindowSystem>()
        .with::<Mut<AssetSystem>>();

    fn new(mut ctx: GeeseContextHandle<Self>) -> Self {
        Self {
            ctx,

            state: GraphicsSystemState::Uninitialized,
        }
    }
}
