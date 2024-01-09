use std::{borrow::Cow, collections::HashMap, hash::BuildHasherDefault};

use geese::*;
use graphics::{GraphicsSystem, WindowSystem};
use log::{debug, info};

use winit::{event_loop::EventLoop, dpi::PhysicalSize};

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
    pub fn create_window(&self, title: &str, size: Option<PhysicalSize<u32>>) {
        let win_sys = self.ctx.get::<WindowSystem>();
        let window = win_sys.window_handle();
        window.set_visible(true);
        window.set_min_inner_size(size);
        window.set_title(title);
    }


    pub fn run(&mut self, mut ctx: GeeseContext) {
        let mut event_loop_sys = self.ctx.get_mut::<EventLoopSystem>();
        let event_loop = event_loop_sys.take();
        drop(event_loop_sys);
        event_loop.run(move |event, target| {
            ctx.flush().with(event);
            self.use_window_target(target);
            self.update();
        }).unwrap();
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
        .with::<EventLoopSystem>()
        .with::<GraphicsSystem>()
        // FIXME
        .with::<WindowSystem>();

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

