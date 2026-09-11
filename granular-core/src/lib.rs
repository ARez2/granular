use rustc_hash::FxHashMap as HashMap;
use std::{
    marker::PhantomData,
    sync::{Arc, atomic::AtomicBool},
};
use web_time::{Duration, Instant};
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalSize, PhysicalSize},
    event::{DeviceEvent, DeviceId, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
    window::WindowId,
};

pub mod future_executor;

mod rect;
pub use rect::Rect;

pub mod utils;
pub use utils::*;

mod time_system;
pub use time_system::TimeSystem;

pub mod assets;
pub use assets::AssetSystem;

//mod tick;
pub mod graphics;
pub use graphics::{BatchRenderer, Camera};
use graphics::{GameRenderer, GraphicsSystem, WindowSystem};

mod filewatcher;
use filewatcher::FileWatcher;

pub mod input_system;
pub use input_system::{InputAction, InputActionTrigger, InputSystem};

pub mod prelude {
    pub use super::{
        AssetSystem, BatchRenderer, Camera, GranularEngine,
        assets::{self, AssetHandle},
        events,
        graphics::{
            self, GraphicsSystem, Texture2D, TextureBundle, TextureBundleLoadSettings, WindowSystem,
        },
        input_system::*,
        rect::Rect,
        time_system::TimeSystem,
        utils::*,
    };
}

pub mod events {
    use winit::dpi::PhysicalSize;

    pub struct Initialized {}

    pub mod timing {
        /// Gets sent out every N frames
        pub struct Tick<const N: u32>;

        /// Gets sent out every T milliseconds
        pub struct FixedTick<const N: u64>;
        pub const FIXED_TICKS: [u64; 5] = [5000, 2500, 1000, 16, 1];
        // Emitted every frame and used to let things tick without the outer application seeing it
        pub(crate) struct InternalTick;
    }

    pub struct Resized {
        pub new_size: PhysicalSize<u32>,
    }
}

enum CustomWinitEvent {
    GraphicsSystemInitialized { state: graphics::GraphicsState },
    WindowResized(PhysicalSize<u32>),
    InitDone,
}

/// Control flow:
///     On winit resumed:  Uninitialized => Preparing
///     if user called WindowSystem::set_window_size() in their Game::new():
///         On CustomWinitEvent::WindowResized:   Preparing => Running
///     else:
///         On winit::new_events (with cause = Poll):  Preparing => Running
///
/// Note: This "if" ensures that the surface/ camera resize is already done by the time the user
/// receives its event::Initialized, so he can use the correct window/surface size from there
#[derive(Debug, PartialEq, Eq, PartialOrd)]
enum EngineState {
    Uninitialized,
    PrepareGraphics,
    PreparingBeforeRun,
    Running,
}

pub struct GranularEngine<AppSystem: GeeseSystem + std::fmt::Debug> {
    ctx: GeeseContext,
    game_resolution: LogicalSize<u32>,
    event_loop: Option<EventLoop<CustomWinitEvent>>,
    event_loop_proxy: EventLoopProxy<CustomWinitEvent>,
    state: EngineState,
    /// See explanation on `EngineState`
    waiting_for_window_event: Arc<AtomicBool>,
    /// Current frame
    frame: u64,
    /// When each tick (in ms) last occured
    last_ticks: HashMap<Duration, Instant>,
    application: PhantomData<AppSystem>,
    last_handled_resize: Option<PhysicalSize<u32>>,
}
#[profiling::all_functions]
impl<AppSystem: GeeseSystem + std::fmt::Debug> GranularEngine<AppSystem> {
    // This game resolution is the size that the game renders at (<= display/ window size)
    pub fn new(game_resolution: LogicalSize<u32>) -> Self {
        let now = Instant::now();
        let mut last_ticks = HashMap::default();
        for fixed_tick in events::timing::FIXED_TICKS {
            last_ticks.insert(Duration::from_millis(fixed_tick), now);
        }

        let mut ctx = GeeseContext::default();
        ctx.flush()
            .with(geese::notify::add_system::<WindowSystem>())
            .with(geese::notify::add_system::<GraphicsSystem>())
            .with(geese::notify::add_system::<FutureExecutor>())
            .with(geese::notify::add_system::<FileWatcher>())
            .with(geese::notify::add_system::<TimeSystem>())
            .with(geese::notify::add_system::<InputSystem>());

        trace!("Core systems added.");

        let event_loop = EventLoop::with_user_event().build().unwrap();
        event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
        let proxy = event_loop.create_proxy();

        Self {
            ctx,
            game_resolution,
            event_loop: Some(event_loop),
            event_loop_proxy: proxy,
            state: EngineState::Uninitialized,
            waiting_for_window_event: Arc::new(AtomicBool::new(false)),
            frame: 0,
            last_ticks,
            application: PhantomData,
            last_handled_resize: None,
        }
    }

    #[profiling::skip]
    pub fn get_ctx(&mut self) -> &mut GeeseContext {
        &mut self.ctx
    }

    /// Invokes the main loop
    #[profiling::skip]
    pub fn run(mut self) {
        #[cfg(feature = "trace")]
        tracy_client::Client::start();

        let event_loop = self
            .event_loop
            .take()
            .expect("Event loop was already taken!");
        #[cfg(target_arch = "wasm32")]
        {
            use winit::platform::web::EventLoopExtWebSys;
            event_loop.spawn_app(self);
        }
        #[cfg(not(target_arch = "wasm32"))]
        event_loop.run_app(&mut self).unwrap();
    }

    pub fn update(&mut self) {}

    /// Responsible for emitting the right `events::timing::Tick` or `events::timing::FixedTick`
    pub fn handle_scheduling(&mut self) {
        let mut buffer = geese::EventBuffer::default().with(events::timing::Tick::<1>);

        let now = Instant::now();
        for (tickrate, last) in &mut self.last_ticks {
            if *last + *tickrate < now {
                *last = now;

                match tickrate.as_millis() as u64 {
                    1 => buffer = buffer.with(events::timing::FixedTick::<1>),
                    16 => buffer = buffer.with(events::timing::FixedTick::<16>),
                    1000 => buffer = buffer.with(events::timing::FixedTick::<1000>),
                    2500 => buffer = buffer.with(events::timing::FixedTick::<2500>),
                    5000 => buffer = buffer.with(events::timing::FixedTick::<5000>),
                    _ => {}
                }
            }
        }

        if self.frame.is_multiple_of(60) {
            buffer = buffer.with(events::timing::Tick::<60>);
        };
        if self.frame.is_multiple_of(30) {
            buffer = buffer.with(events::timing::Tick::<30>);
        };
        if self.frame.is_multiple_of(10) {
            buffer = buffer.with(events::timing::Tick::<10>);
        };
        if self.frame.is_multiple_of(2) {
            buffer = buffer.with(events::timing::Tick::<2>);
        };
        // 1 Frame tick is already handled at the very top

        self.ctx.flush().with_buffer(buffer);
    }

    /// Resizes the surface with the new_size
    fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if let Some(size) = self.last_handled_resize
            && size == new_size
        {
            trace!("Resize of this size was already handled before! Returning...");
            return;
        }
        {
            let mut graphics_sys = self.ctx.get_mut::<GraphicsSystem>();
            graphics_sys.resize_surface(new_size);
            #[cfg(target_os = "macos")]
            graphics_sys.request_redraw();
        }
        {
            let mut camera = self.ctx.get_mut::<Camera>();
            camera.set_screen_size((new_size.width, new_size.height));
        }
        self.last_handled_resize = Some(new_size);
        self.ctx.flush().with(events::Resized { new_size });
    }
}
#[profiling::all_functions]
// Implement the winit::ApplicationHandler trait
impl<AppSystem: GeeseSystem + std::fmt::Debug> ApplicationHandler<CustomWinitEvent>
    for GranularEngine<AppSystem>
{
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        {
            let mut window_sys = self.ctx.get_mut::<WindowSystem>();
            window_sys.init(
                event_loop,
                self.event_loop_proxy.clone(),
                self.waiting_for_window_event.clone(),
            );
        }
        {
            let mut graphics_sys = self.ctx.get_mut::<GraphicsSystem>();
            graphics_sys.init(
                event_loop,
                self.event_loop_proxy.clone(),
                self.game_resolution,
            );
        }
        self.state = EngineState::PrepareGraphics;
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: CustomWinitEvent) {
        match event {
            CustomWinitEvent::GraphicsSystemInitialized { state } => {
                let mut graphics_sys = self.ctx.get_mut::<GraphicsSystem>();
                graphics_sys.initialize_callback(state);
                drop(graphics_sys);

                self.state = EngineState::PreparingBeforeRun;
                trace!("Graphics initialized");

                self.ctx
                    .flush()
                    .with(geese::notify::add_system::<AssetSystem>())
                    .with(geese::notify::add_system::<Camera>())
                    .with(geese::notify::add_system::<GameRenderer>())
                    .with(geese::notify::delayed(
                        geese::notify::add_system::<AppSystem>(),
                    ));
            }
            CustomWinitEvent::WindowResized(new_size) => {
                self.resize(new_size);
                // See explanation on `EngineState`
                if self.state != EngineState::Running
                    && self
                        .waiting_for_window_event
                        .load(std::sync::atomic::Ordering::Relaxed)
                {
                    debug!("Send initDone from resized");
                    let _ = self.event_loop_proxy.send_event(CustomWinitEvent::InitDone);
                }
            }
            CustomWinitEvent::InitDone => {
                if self.state == EngineState::PreparingBeforeRun {
                    self.ctx.flush().with(events::Initialized {});
                    self.state = EngineState::Running;
                }
            }
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        info!("Exiting...");
    }

    // This cause will pretty much always be winit::event::StartCause::Poll
    fn new_events(&mut self, _event_loop: &ActiveEventLoop, _cause: winit::event::StartCause) {
        self.ctx.flush().with(events::timing::InternalTick);
        // Only run the following if the Graphics, Window etc. are initialized
        if self.state != EngineState::Running {
            // See explanation on `EngineState`
            if self.state == EngineState::PreparingBeforeRun {
                {
                    self.ctx.get::<GraphicsSystem>().request_redraw();
                }
                if !self
                    .waiting_for_window_event
                    .load(std::sync::atomic::Ordering::Relaxed)
                {
                    let _ = self.event_loop_proxy.send_event(CustomWinitEvent::InitDone);
                }
            }
            return;
        }

        {
            let mut input = self.ctx.get_mut::<InputSystem>();
            input.reset_just_pressed();
        }
        self.update();
        self.handle_scheduling();
        self.ctx.get_mut::<TimeSystem>().mark_frame();
        self.frame += 1;
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        if self.state < EngineState::PreparingBeforeRun {
            return;
        }

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(new_size) => {
                self.resize(new_size);
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                let mut input = self.ctx.get_mut::<InputSystem>();
                input.update_modifiers(&modifiers);
            }
            WindowEvent::RedrawRequested => {
                {
                    let camera = self.ctx.get::<Camera>();
                    camera.write_canvas_transform_buffer();
                }
                {
                    let mut graphics_sys = self.ctx.get_mut::<GraphicsSystem>();
                    graphics_sys.start_rendering();
                }

                profiling::finish_frame!();
            }
            WindowEvent::KeyboardInput {
                event,
                is_synthetic: false,
                ..
            } => {
                let mut input = self.ctx.get_mut::<InputSystem>();
                input.handle_keyevent(&event);
            }
            WindowEvent::CursorMoved { position, .. } => {
                let mut input = self.ctx.get_mut::<InputSystem>();
                input.handle_cursor_movement(position);
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let mut input = self.ctx.get_mut::<InputSystem>();
                input.handle_mouse_input(button, state);
            }
            WindowEvent::MouseWheel {
                device_id: _,
                delta: _,
                phase: _,
            } => {
                // TODO: input.handle_mouse_wheel()
            }

            WindowEvent::CursorLeft { .. }
            | WindowEvent::TouchpadPressure { .. }
            | WindowEvent::HoveredFileCancelled
            | WindowEvent::KeyboardInput { .. }
            | WindowEvent::CursorEntered { .. }
            | WindowEvent::AxisMotion { .. }
            | WindowEvent::DroppedFile(_)
            | WindowEvent::HoveredFile(_)
            | WindowEvent::Destroyed
            | WindowEvent::Touch(_)
            | WindowEvent::Moved(_)
            | WindowEvent::DoubleTapGesture { .. }
            | WindowEvent::PanGesture { .. }
            | WindowEvent::RotationGesture { .. }
            | WindowEvent::PinchGesture { .. }
            | WindowEvent::Ime(_)
            | WindowEvent::ActivationTokenDone { .. }
            | WindowEvent::Occluded(_)
            | WindowEvent::Focused(_)
            | WindowEvent::ScaleFactorChanged { .. }
            | WindowEvent::ThemeChanged(_) => {
                self.ctx.flush().with(event);
            }
        };
    }

    #[profiling::skip]
    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: DeviceId,
        _event: DeviceEvent,
    ) {
        // info!("Device {device_id:?} event: {event:?}");
    }
}
