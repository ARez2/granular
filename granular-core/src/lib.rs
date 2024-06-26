use std::{marker::PhantomData, time::{Duration, Instant}};

use geese::{EventQueue, GeeseContext, GeeseSystem};
use log::info;
use rustc_hash::FxHashMap as HashMap;
use winit::{application::ApplicationHandler, dpi::PhysicalSize, event::{DeviceEvent, DeviceId, WindowEvent}, event_loop::ActiveEventLoop, window::WindowId};

pub mod assets;
pub use assets::AssetSystem;

//mod tick;
pub mod graphics;
pub use graphics::{BatchRenderer, Camera};
use graphics::{Renderer, WindowSystem};

mod eventloop_system;
pub use eventloop_system::EventLoopSystem;

mod filewatcher;
use filewatcher::FileWatcher;

pub mod input_system;
pub use input_system::{InputSystem, InputActionTrigger, InputAction};

pub mod simulation;
pub use simulation::*;


pub mod events {
    pub struct Initialized {
        
    }

    pub mod timing {
        /// Gets sent out every N frames
        pub struct Tick<const N: u32>;

        /// Gets sent out every T milliseconds
        pub struct FixedTick<const N: u64>;
        pub const FIXED_TICKS: [u64; 3] = [5000, 2500, 1000];
    }

    pub struct Draw;
}




pub struct GranularEngine<AppSystem: GeeseSystem> {
    ctx: GeeseContext,
    close_requested: bool,
    /// Current frame
    frame: u64,
    /// When each tick (in ms) last occured
    last_ticks: HashMap<Duration, Instant>,
    application: PhantomData<AppSystem>
}

impl<AppSystem: GeeseSystem> GranularEngine<AppSystem> {
    pub fn new() -> Self {
        let mut ctx: GeeseContext = GeeseContext::default();
        ctx.flush()
            .with(geese::notify::add_system::<WindowSystem>())
            .with(geese::notify::add_system::<EventLoopSystem>())
            .with(geese::notify::add_system::<FileWatcher>())
            .with(geese::notify::add_system::<InputSystem>());

        let now = Instant::now();
        let mut last_ticks = HashMap::default();
        for fixed_tick in events::timing::FIXED_TICKS {
            last_ticks.insert(Duration::from_millis(fixed_tick), now);
        };

        Self {
            ctx,
            close_requested: false,
            frame: 0,
            last_ticks,
            application: PhantomData
        }
    }


    pub fn get_ctx(&mut self) -> &mut GeeseContext {
        &mut self.ctx
    }


    pub fn run(&mut self) {
        info!("GranularEngine run");
        let mut event_loop_sys = self.ctx.get_mut::<EventLoopSystem>();
        let event_loop = event_loop_sys.take();
        drop(event_loop_sys);
        event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
        let _ = event_loop.run_app(self);
    }


    pub fn update(&mut self) {

    }


    pub fn handle_scheduling(&mut self) {
        let mut buffer = geese::EventBuffer::default()
            .with(events::timing::Tick::<1>);
        
        let now = Instant::now();
        self.last_ticks.iter_mut().for_each(|(tickrate, last)| {
            if *last + *tickrate < now {
                *last = now;
                let tickrate_millis = tickrate.as_millis() as u64;
                match tickrate_millis {
                    1000 => {self.ctx.flush().with(events::timing::FixedTick::<1000>);},
                    2500 => {self.ctx.flush().with(events::timing::FixedTick::<2500>);},
                    5000 => {self.ctx.flush().with(events::timing::FixedTick::<5000>);},
                    _ => ()
                };
            }
        });

        if self.frame % 60 == 0 {
            buffer = buffer.with(events::timing::Tick::<60>);
        };
        if self.frame % 30 == 0 {
            buffer = buffer.with(events::timing::Tick::<30>);
        };
        if self.frame % 2 == 0 {
            buffer = buffer.with(events::timing::Tick::<2>);
        };
        // 1 Frame tick is already handled at the very top
        
        self.ctx.flush().with_buffer(buffer);
    }
}
impl<AppSystem: GeeseSystem> ApplicationHandler for GranularEngine<AppSystem> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        info!("Resumed!");
        {
            let mut window_sys = self.ctx.get_mut::<WindowSystem>();
            window_sys.init(event_loop);
        }
        self.ctx.flush()
            .with(geese::notify::add_system::<Renderer>())
            .with(geese::notify::add_system::<AssetSystem>())
            .with(geese::notify::add_system::<AppSystem>())
            .with(events::Initialized{});
        
    }


    fn exiting(&mut self, event_loop: &ActiveEventLoop) {
        info!("Exiting...");
    }


    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: winit::event::StartCause) {
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
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            },
            WindowEvent::Resized(new_size) => {
                let mut renderer = self.ctx.get_mut::<Renderer>();
                renderer.resize(new_size);
                #[cfg(target_os="macos")]
                graphics.request_redraw();
            },
            WindowEvent::ModifiersChanged(modifiers) => {
                let mut input = self.ctx.get_mut::<InputSystem>();
                input.update_modifiers(&modifiers);
            },
            WindowEvent::RedrawRequested => {
                self.ctx.flush().with(events::Draw);
                let mut renderer = self.ctx.get_mut::<Renderer>();
                renderer.start_frame();
                renderer.render();
                renderer.end_frame();
                renderer.request_redraw();
            },
            WindowEvent::KeyboardInput{event, is_synthetic: false, ..} => {
                let mut input = self.ctx.get_mut::<InputSystem>();
                input.handle_keyevent(&event);
            },
            WindowEvent::CursorMoved {position, .. } => {
                let mut input = self.ctx.get_mut::<InputSystem>();
                input.handle_cursor_movement(position);
            },
            WindowEvent::MouseInput {state, button, ..} => {
                let mut input = self.ctx.get_mut::<InputSystem>();
                input.handle_mouse_input(button, state);
            },
            WindowEvent::MouseWheel { device_id, delta, phase } => {

            },
            
            
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
            | WindowEvent::PanGesture{ .. }
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


    fn device_event(
            &mut self,
            event_loop: &ActiveEventLoop,
            device_id: DeviceId,
            event: DeviceEvent,
        ) {
        //info!("Device {device_id:?} event: {event:?}");
    }
}