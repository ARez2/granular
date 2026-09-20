#![allow(unused)]

use std::sync::Arc;
#[cfg(feature = "trace")]
use std::sync::Mutex;

use super::view_mapping::game_target_size;
use anyhow::bail;
use bytemuck_derive::{Pod, Zeroable};
use glam::{UVec2, Vec2};
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
    graphics::{Texture2D, TextureBundle, TextureHandle},
    utils::*,
};

pub mod events {
    pub struct GameResolutionChanged {}

    pub struct RunSimulation {}
    pub struct AfterSimulation {}
    pub struct RecordGameRenderingCommands {}
    /// Event for internal renderers to dispatch commands to for ex. the BatchRenderer
    pub(crate) struct RecordInternalGameRenderingCommands {}
    /// Freeze CPU transforms and upload uniforms after updates, before drawing.
    pub(crate) struct PrepareRender {}
    pub struct RenderGame {}
    pub struct GameRenderingDone {}
    pub(crate) struct DisplayGame {}
    pub(crate) struct DisplayGameDone {}
    pub struct RecordUiRenderingCommands {}
    pub(crate) struct RecordInternalUiRenderingCommands {}
    pub struct RenderUi {}
    pub(crate) struct UiRenderingDone {}
}

pub struct RenderContext {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    frame: SurfaceTexture,
    pub view: TextureView,
    pub encoder: CommandEncoder,
    #[cfg(feature = "trace")]
    pub profiler: Arc<Mutex<GpuProfiler>>,
}

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
    context: Option<RenderContext>,
    game_resolution: UVec2,
    game_texture_handle: Option<TextureHandle>,
}
#[profiling::all_functions]
impl GraphicsSystem {
    pub(crate) fn init(
        &mut self,
        event_loop: &ActiveEventLoop,
        proxy: EventLoopProxy<CustomWinitEvent>,
        game_resolution: UVec2,
    ) {
        if !matches!(self.state, GraphicsSystemState::Uninitialized) {
            return;
        }
        self.state = GraphicsSystemState::Loading;
        self.game_resolution = game_resolution;

        let window_sys = self.ctx.get::<WindowSystem>();
        let window = window_sys.window_handle();
        drop(window_sys);
        let display_handle = event_loop.owned_display_handle();

        let mut executor = self.ctx.get_mut::<FutureExecutor>();
        executor.spawn_oneshot(async move {
            let mut inst_desc = wgpu::InstanceDescriptor::new_with_display_handle_from_env(
                Box::new(display_handle),
            );

            inst_desc.flags = wgpu::InstanceFlags::from_build_config().with_env();

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

            // debug!("{:?}", surface.get_capabilities(&adapter).formats);

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
                .get_default_config(
                    &adapter,
                    window.inner_size().width,
                    window.inner_size().height,
                )
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

            let _ = proxy.send_event(CustomWinitEvent::GraphicsSystemInitialized {
                state: GraphicsState {
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
                },
            });
        });
    }

    pub(crate) fn initialize_callback(&mut self, state: GraphicsState) {
        self.state = GraphicsSystemState::Ready(state);
        self.set_game_resolution(self.game_resolution);

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
                let scope = dev.push_error_scope(wgpu::ErrorFilter::Validation);

                let module = dev.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: None,
                    source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Owned(String::from_utf8(
                        bytes,
                    )?)),
                });

                if pollster::block_on(scope.pop()).is_some() {
                    return Err(anyhow::anyhow!("Error while reloading asset!"));
                }

                Ok(module)
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
                    Some((
                        &bytes,
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(
                                crate::graphics::bytes_per_pixel(settings.format).unwrap_or(4)
                                    * settings.size.width,
                            ),
                            rows_per_image: Some(settings.size.height),
                        },
                    )),
                ))
            });
        }
    }

    pub(crate) fn request_redraw(&self) {
        self.ctx
            .get::<WindowSystem>()
            .window_handle()
            .request_redraw();
    }

    /// Creates a new TextureBundle which the game will render to
    fn create_game_render_target(
        device: &Device,
        queue: &Queue,
        mut format: wgpu::TextureFormat,
        game_resolution: UVec2,
    ) -> TextureBundle {
        format = format.add_srgb_suffix();
        let target_size = game_target_size(game_resolution);
        TextureBundle::new(
            device,
            queue,
            "Game render target",
            wgpu::TextureDescriptor {
                label: Some("Game render target desc"),
                size: wgpu::Extent3d {
                    width: target_size.x,
                    height: target_size.y,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            &wgpu::TextureViewDescriptor::default(),
            &wgpu::SamplerDescriptor {
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Nearest,
                min_filter: wgpu::FilterMode::Nearest,
                mipmap_filter: wgpu::MipmapFilterMode::Nearest,
                ..Default::default()
            },
            None,
        )
    }

    pub fn get_game_target_size(&self) -> UVec2 {
        game_target_size(self.game_resolution)
    }

    pub fn get_game_render_target(&self) -> TextureHandle {
        self.game_texture_handle.clone().expect("Should exist")
    }

    pub fn get_surface_resolution(&self) -> PhysicalSize<u32> {
        if let GraphicsSystemState::Ready(state) = &self.state {
            return PhysicalSize::new(state.surface_config.width, state.surface_config.height);
        }
        panic!("GraphicsSystem is not ready!");
    }

    /// Visible game resolution, excluding the one-texel border on each side.
    pub fn get_game_resolution(&self) -> UVec2 {
        self.game_resolution
    }

    /// Change during update, before PrepareRender. Allocation includes overscan.
    pub fn set_game_resolution(&mut self, mut game_resolution: UVec2) {
        game_resolution = game_resolution.max(UVec2::ONE);

        self.game_resolution = game_resolution;
        let GraphicsSystemState::Ready(state) = &mut self.state else {
            return;
        };
        let target_size = game_target_size(game_resolution);
        let limit = state.device.limits().max_texture_dimension_2d;
        assert!(
            target_size.x <= limit && target_size.y <= limit,
            "game resolution including overscan exceeds the device texture limit"
        );
        let mut texture = None;
        if let GraphicsSystemState::Ready(state) = &mut self.state {
            texture = Some(Self::create_game_render_target(
                &state.device,
                &state.queue,
                state.surface_config.format,
                game_resolution,
            ));
        }
        if let Some(tex) = texture {
            let handle = {
                let mut asset_sys = self.ctx.get_mut::<AssetSystem>();
                asset_sys.register(tex)
            };
            self.game_texture_handle = Some(handle);
        }
        self.ctx.raise_event(events::GameResolutionChanged {});
    }

    pub fn resize_surface(&mut self, new_size: PhysicalSize<u32>) {
        let GraphicsSystemState::Ready(state) = &mut self.state else {
            return;
        };

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

    fn begin_frame(
        &mut self,
        existing_ctx: Option<RenderContext>,
        frame_name: &str,
        render_to_game: bool,
    ) {
        self.device().poll(wgpu::wgt::PollType::Poll);
        let GraphicsSystemState::Ready(state) = &mut self.state else {
            error!("GraphicsSystem is not ready!");
            return;
        };

        let window = self.ctx.get::<WindowSystem>().window_handle();

        // always create a new encoder to ensure ordering
        let encoder = state
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some(&format!("{} command encoder", frame_name)),
            });

        if let Some(ctx) = existing_ctx {
            let frame = ctx.frame;
            let view = if render_to_game {
                let asset_sys = self.ctx.get::<AssetSystem>();
                let tex = asset_sys
                    .get(self.game_texture_handle.as_ref().unwrap())
                    .unwrap();
                tex.view().clone()
            } else {
                frame.texture.create_view(&wgpu::TextureViewDescriptor {
                    format: Some(Self::calculate_surface_view_format(
                        &state.surface_config.format,
                    )),
                    ..Default::default()
                })
            };

            ctx.queue.submit(Some(ctx.encoder.finish()));
            self.context = Some(RenderContext {
                device: ctx.device,
                queue: ctx.queue,
                frame,
                view,
                encoder,
                #[cfg(feature = "trace")]
                profiler: ctx.profiler,
            });
        } else {
            let frame = match state.surface.get_current_texture() {
                CurrentSurfaceTexture::Success(frame) => frame,
                CurrentSurfaceTexture::Timeout | CurrentSurfaceTexture::Occluded => {
                    // Try again later
                    window.request_redraw();
                    error!("Surface got a timeout or is occluded. Try again later.");
                    return;
                }
                CurrentSurfaceTexture::Suboptimal(texture) => {
                    drop(texture);

                    state
                        .surface
                        .configure(&state.device, &state.surface_config);
                    window.request_redraw();
                    error!("Surface isnt optimal. Try again next frame.");
                    return;
                }
                CurrentSurfaceTexture::Outdated => {
                    state
                        .surface
                        .configure(&state.device, &state.surface_config);
                    window.request_redraw();
                    error!("The surface is outdated. Try again next frame.");
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
                    error!("The surface has been lost. Try again next frame.");
                    return;
                }
            };
            let view = if render_to_game {
                let asset_sys = self.ctx.get::<AssetSystem>();
                let tex = asset_sys
                    .get(self.game_texture_handle.as_ref().unwrap())
                    .unwrap();
                tex.view().clone()
            } else {
                frame.texture.create_view(&wgpu::TextureViewDescriptor {
                    format: Some(Self::calculate_surface_view_format(
                        &state.surface_config.format,
                    )),
                    ..Default::default()
                })
            };

            self.context = Some(RenderContext {
                device: state.device.clone(),
                queue: state.queue.clone(),
                frame,
                view,
                encoder,
                #[cfg(feature = "trace")]
                profiler: state.profiler.clone(),
            });
        }
    }

    pub fn device(&self) -> &Device {
        if let GraphicsSystemState::Ready(state) = &self.state {
            return &state.device;
        }
        panic!("GraphicsSystem is not ready!");
    }

    pub fn render_context(&mut self) -> &mut RenderContext {
        self.context.as_mut().expect("Context must exist")
    }

    pub(crate) fn start_frame(&mut self) {
        self.begin_frame(None, "Simulation", true);
        if !matches!(self.state, GraphicsSystemState::Ready(_)) || self.context.is_none() {
            return;
        }
        self.ctx
            .raise_event(geese::notify::flush().with(events::RunSimulation {}));
        self.ctx
            .raise_event(geese::notify::flush().with(events::AfterSimulation {}));
    }

    fn start_game_render(&mut self, _: &events::AfterSimulation) {
        let ctx = self.context.take();
        self.begin_frame(ctx, "Game render", true);
        self.ctx
            .raise_event(geese::notify::flush().with(events::PrepareRender {}));

        self.ctx.raise_event(
            geese::notify::flush()
                .with(events::RecordGameRenderingCommands {})
                .with(events::RecordInternalGameRenderingCommands {}),
        );
        self.ctx
            .raise_event(geese::notify::flush().with(events::RenderGame {}));
        self.ctx
            .raise_event(geese::notify::flush().with(events::GameRenderingDone {}));
    }

    fn start_display_game_render(&mut self, _: &events::GameRenderingDone) {
        let ctx = self.context.take();
        self.begin_frame(ctx, "Display game", false);

        self.ctx
            .raise_event(geese::notify::flush().with(events::DisplayGame {}));
        self.ctx
            .raise_event(geese::notify::flush().with(events::DisplayGameDone {}));
    }

    fn start_ui_render(&mut self, _: &events::DisplayGameDone) {
        let ctx = self.context.take();
        self.begin_frame(ctx, "UI render", false);

        self.ctx.raise_event(
            geese::notify::flush()
                .with(geese::notify::flush().with(events::RecordUiRenderingCommands {}))
                .with(geese::notify::flush().with(events::RecordInternalUiRenderingCommands {})),
        );
        self.ctx
            .raise_event(geese::notify::flush().with(events::RenderUi {}));
        self.ctx
            .raise_event(geese::notify::flush().with(events::UiRenderingDone {}));
    }

    fn finish_frame(&mut self, _: &events::UiRenderingDone) {
        if let GraphicsSystemState::Ready(state) = &mut self.state {
            let mut context = self.context.take().unwrap();

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

        self.request_redraw();
        profiling::finish_frame!();
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

    pub fn get_game_view_format(&self) -> wgpu::TextureFormat {
        if let GraphicsSystemState::Ready(state) = &self.state {
            return self
                .ctx
                .get::<AssetSystem>()
                .get(self.game_texture_handle.as_ref().unwrap())
                .unwrap()
                .view()
                .texture()
                .format();
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
}
#[profiling::all_functions]
impl GeeseSystem for GraphicsSystem {
    const DEPENDENCIES: Dependencies = dependencies()
        .with::<Mut<FutureExecutor>>()
        .with::<WindowSystem>()
        .with::<Mut<AssetSystem>>();

    const EVENT_HANDLERS: EventHandlers<Self> = event_handlers()
        .with(Self::start_game_render)
        .with(Self::start_display_game_render)
        .with(Self::start_ui_render)
        .with(Self::finish_frame);

    fn new(mut ctx: GeeseContextHandle<Self>) -> Self {
        Self {
            ctx,
            state: GraphicsSystemState::Uninitialized,
            context: None,
            game_resolution: UVec2::ONE,
            game_texture_handle: None,
        }
    }
}
impl Drop for GraphicsSystem {
    fn drop(&mut self) {
        // drop the context before the graphics state to hopefully fix
        // "Trying to destroy a SwapchainAcquireSemaphore that is still in use by a SurfaceTexture"
        // which occured on shutdown sometimes
        drop(self.context.take());
    }
}
