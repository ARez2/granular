use rustc_hash::FxHashMap as HashMap;
use std::marker::PhantomData;
use web_time::{Duration, Instant};
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, DeviceId, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
    window::WindowId,
};

pub mod future_executor;

pub mod utils;
pub use utils::*;

pub mod assets;
pub use assets::AssetSystem;

//mod tick;
pub mod graphics;
pub use graphics::{BatchRenderer, Camera};
use graphics::{Renderer, WindowSystem};

mod filewatcher;
use filewatcher::FileWatcher;

pub mod input_system;
pub use input_system::{InputAction, InputActionTrigger, InputSystem};

// pub mod simulation;
// pub use simulation::*;

use crate::graphics::GraphicsSystem;

pub mod events {
    pub struct Initialized {}

    pub mod timing {
        /// Gets sent out every N frames
        pub struct Tick<const N: u32>;

        /// Gets sent out every T milliseconds
        pub struct FixedTick<const N: u64>;
        pub const FIXED_TICKS: [u64; 5] = [5000, 2500, 1000, 16, 1];
    }

    pub struct Draw;
}

enum CustomWinitEvent {
    GraphicsSystemInitialized(graphics::GraphicsState),
}

#[derive(Debug, PartialEq, Eq)]
enum EngineState {
    Uninitialized,
    Preparing,
    Running,
}

#[derive(Debug)]
pub struct GranularEngine<AppSystem: GeeseSystem + std::fmt::Debug> {
    ctx: GeeseContext,
    event_loop: Option<EventLoop<CustomWinitEvent>>,
    event_loop_proxy: EventLoopProxy<CustomWinitEvent>,
    state: EngineState,
    /// Current frame
    frame: u64,
    /// When each tick (in ms) last occured
    last_ticks: HashMap<Duration, Instant>,
    application: PhantomData<AppSystem>,
}
impl<AppSystem: GeeseSystem + std::fmt::Debug> Default for GranularEngine<AppSystem> {
    fn default() -> Self {
        Self::new()
    }
}
#[profiling::all_functions]
impl<AppSystem: GeeseSystem + std::fmt::Debug> GranularEngine<AppSystem> {
    pub fn new() -> Self {
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
            .with(geese::notify::add_system::<InputSystem>());

        info!("Core systems added.");

        let event_loop = EventLoop::with_user_event().build().unwrap();
        event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
        let proxy = event_loop.create_proxy();

        Self {
            ctx,
            event_loop: Some(event_loop),
            event_loop_proxy: proxy,
            state: EngineState::Uninitialized,
            frame: 0,
            last_ticks,
            application: PhantomData,
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
}
#[profiling::all_functions]
// Implement the winit::ApplicationHandler trait
impl<AppSystem: GeeseSystem + std::fmt::Debug> ApplicationHandler<CustomWinitEvent>
    for GranularEngine<AppSystem>
{
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.state = EngineState::Preparing;

        {
            let mut window_sys = self.ctx.get_mut::<WindowSystem>();
            window_sys.init(event_loop);
        }
        {
            let mut graphics_sys = self.ctx.get_mut::<GraphicsSystem>();
            graphics_sys.init(event_loop, self.event_loop_proxy.clone());
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: CustomWinitEvent) {
        match event {
            CustomWinitEvent::GraphicsSystemInitialized(graphics_state) => {
                let mut graphics_sys = self.ctx.get_mut::<GraphicsSystem>();
                graphics_sys.initialize_callback(graphics_state);
                drop(graphics_sys);

                self.ctx
                    .flush()
                    .with(geese::notify::add_system::<Renderer>())
                    .with(geese::notify::add_system::<AssetSystem>())
                    // .with(geese::notify::add_system::<Simulation>())
                    .with(geese::notify::add_system::<AppSystem>())
                    .with(events::Initialized {});
                self.state = EngineState::Running;
                info!("Everything is initialized.");

                {
                    let win = self.ctx.get::<WindowSystem>().window_handle();
                    self.ctx.get_mut::<Renderer>().resize(win.inner_size());
                }
            }
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        info!("Exiting...");
    }

    fn new_events(&mut self, _event_loop: &ActiveEventLoop, _cause: winit::event::StartCause) {
        // We still need the scheduling to drive for example the FutureExecutor
        if self.state != EngineState::Running {
            self.handle_scheduling();
            return;
        }
        {
            let mut input = self.ctx.get_mut::<InputSystem>();
            input.reset_just_pressed();
        }
        self.update();
        self.handle_scheduling();
        self.frame += 1;
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        if self.state != EngineState::Running {
            return;
        }

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(new_size) => {
                let mut renderer = self.ctx.get_mut::<Renderer>();
                renderer.resize(new_size);
                #[cfg(target_os = "macos")]
                graphics.request_redraw();
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                let mut input = self.ctx.get_mut::<InputSystem>();
                input.update_modifiers(&modifiers);
            }
            WindowEvent::RedrawRequested => {
                self.ctx.flush().with(events::Draw);
                let mut renderer = self.ctx.get_mut::<Renderer>();
                renderer.render();
                renderer.request_redraw();

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
