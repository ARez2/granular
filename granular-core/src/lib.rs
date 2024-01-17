use geese::{GeeseContext, EventQueue};
use graphics::{WindowSystem, Renderer2D};
use log::{debug, info};

use notify::EventHandler;
use ::notify::PollWatcher;
use winit::{dpi::PhysicalSize, event::WindowEvent};

//mod tick;
mod graphics;

mod eventloop_system;
pub use eventloop_system::EventLoopSystem;

mod filewatcher;
use filewatcher::FileWatcher;


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
    ctx: GeeseContext,
    close_requested: bool,
}

impl GranularEngine {
    pub fn new() -> Self {
        let mut ctx = GeeseContext::default();
        ctx.flush()
            .with(geese::notify::add_system::<EventLoopSystem>())
            .with(geese::notify::add_system::<Renderer2D>())
            .with(geese::notify::add_system::<WindowSystem>())
            .with(geese::notify::add_system::<FileWatcher>());

        let mut filewatcher = ctx.get_mut::<FileWatcher>();
        let shaders = std::env::current_dir().unwrap().join("shaders");
        filewatcher.watch(shaders, true);
        drop(filewatcher);

        Self {
            ctx,
            close_requested: false
        }
    }


    pub fn get_ctx(&mut self) -> &mut GeeseContext {
        &mut self.ctx
    }


    pub fn create_window(&self, title: &str, size: Option<PhysicalSize<u32>>) {
        let win_sys = self.ctx.get::<WindowSystem>();
        let window = win_sys.window_handle();
        window.set_visible(true);
        window.set_min_inner_size(size);
        window.set_title(title);
    }


    pub fn run(&mut self) {
        let mut event_loop_sys = self.ctx.get_mut::<EventLoopSystem>();
        let event_loop = event_loop_sys.take();
        drop(event_loop_sys);
        event_loop.run(move |event, target| {
            let handled = self.handle_winit_events(&event);
            if !handled {
                self.ctx.flush().with(event);
            };
            self.use_window_target(target);
            self.update();
            let filewatcher = self.ctx.get::<FileWatcher>();
            filewatcher.poll();
        }).unwrap();
    }


    pub fn update(&mut self) {
        self.ctx.flush().with(events::NewFrame {delta: 0.0});
    }


    pub fn handle_winit_events(&mut self, event: &winit::event::Event<()>) -> bool {
        if let winit::event::Event::WindowEvent {
            window_id: _,
            event,
        } = event {
            match event {
                WindowEvent::CloseRequested => {
                    self.close_requested = true;
                    true
                },
                WindowEvent::Resized(new_size) => {
                    let mut renderer = self.ctx.get_mut::<Renderer2D>();
                    renderer.resize(new_size);
                    #[cfg(target_os="macos")]
                    graphics.request_redraw();
                    true
                },
                WindowEvent::RedrawRequested => {
                    let mut renderer = self.ctx.get_mut::<Renderer2D>();
                    renderer.render();
                    renderer.request_redraw();
                    true
                },
                WindowEvent::KeyboardInput{event, ..} => {
                    match event {
                        winit::event::KeyEvent {
                            logical_key: winit::keyboard::Key::Named(winit::keyboard::NamedKey::Space),
                            state: winit::event::ElementState::Pressed,
                            ..
                        } => {
                            info!("Reload GraphicsSystem");
                            //self.ctx.flush().with(geese::notify::reset_system::<GraphicsSystem>());
                            true
                        },
                        _ => false
                    }
                }
                _ => false
            }
        } else {
            false
        }
    }


    pub fn use_window_target(&self, target: &winit::event_loop::EventLoopWindowTarget<()>) {
        if self.close_requested {
            target.exit();
        }
    }
}
