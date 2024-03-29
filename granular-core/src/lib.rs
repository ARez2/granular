use std::time::{Duration, Instant};

use geese::{GeeseContext, EventQueue};
use rustc_hash::FxHashMap as HashMap;
use winit::{dpi::PhysicalSize, event::WindowEvent};

pub mod assets;
pub use assets::AssetSystem;

//mod tick;
pub mod graphics;
pub use graphics::{Renderer, Camera};
use graphics::WindowSystem;

mod eventloop_system;
pub use eventloop_system::EventLoopSystem;

mod filewatcher;
use filewatcher::FileWatcher;

pub mod input_system;
pub use input_system::{InputSystem, InputActionTrigger, InputAction};


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




pub struct GranularEngine {
    ctx: GeeseContext,
    close_requested: bool,
    /// Current frame
    frame: u64,
    /// When each tick (in ms) last occured
    last_ticks: HashMap<Duration, Instant>,

    tex: assets::AssetHandle<assets::TextureAsset>,
    tex2: assets::AssetHandle<assets::TextureAsset>,
    tex3: assets::AssetHandle<assets::TextureAsset>,
}

impl GranularEngine {
    pub fn new() -> Self {
        let mut ctx = GeeseContext::default();
        ctx.flush()
            .with(geese::notify::add_system::<EventLoopSystem>())
            .with(geese::notify::add_system::<Renderer>())
            .with(geese::notify::add_system::<WindowSystem>())
            .with(geese::notify::add_system::<FileWatcher>())
            .with(geese::notify::add_system::<AssetSystem>())
            .with(geese::notify::add_system::<InputSystem>());

        let now = Instant::now();
        let mut last_ticks = HashMap::default();
        for fixed_tick in events::timing::FIXED_TICKS {
            last_ticks.insert(Duration::from_millis(fixed_tick), now);
        };

        let mut asset_sys = ctx.get_mut::<AssetSystem>();
        let tex = asset_sys.load::<assets::TextureAsset>("assets/cat.jpg", true);
        let tex2 = asset_sys.load::<assets::TextureAsset>("assets/cat2.jpg", true);
        let tex3 = asset_sys.load::<assets::TextureAsset>("assets/cat3.jpg", true);
        drop(asset_sys);

        Self {
            ctx,
            close_requested: false,
            frame: 0,
            last_ticks,
            tex,
            tex2,
            tex3
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
            {
                let mut input = self.ctx.get_mut::<InputSystem>();
                input.reset_just_pressed();
            }
            let handled = self.handle_winit_events(&event);
            if !handled {
                self.ctx.flush().with(event);
            };
            //self.use_window_target(target);
            if self.close_requested {
                target.exit();
            };
            self.update();
            self.handle_scheduling();
            self.frame += 1;
        }).unwrap();
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


    pub fn handle_winit_events(&mut self, event: &winit::event::Event<()>) -> bool {
        let mut handled = true;
        if let winit::event::Event::WindowEvent {
            window_id: _,
            event,
        } = event {
            match event {
                WindowEvent::CloseRequested => {
                    self.close_requested = true;
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
                    let mut renderer = self.ctx.get_mut::<Renderer>();
                    renderer.start_frame();
                    drop(renderer);

                    self.ctx.flush().with(events::Draw);

                    let mut renderer = self.ctx.get_mut::<Renderer>();
                    renderer.flush();
                    renderer.end_frame();
                    renderer.request_redraw();
                },
                WindowEvent::KeyboardInput{event, is_synthetic: false, ..} => {
                    let mut input = self.ctx.get_mut::<InputSystem>();
                    input.handle_keyevent(event);
                    let speed = 10;
                    // match event {
                    //     winit::event::KeyEvent {
                    //         logical_key: winit::keyboard::Key::Named(winit::keyboard::NamedKey::ArrowUp),
                    //         state: winit::event::ElementState::Pressed,
                    //         ..
                    //     } => {
                    //         let mut renderer = self.ctx.get_mut::<Renderer>();
                    //         renderer.camera.translate(IVec2::new(0, 1) * speed);
                    //         true
                    //     },
                    //     winit::event::KeyEvent {
                    //         logical_key: winit::keyboard::Key::Named(winit::keyboard::NamedKey::ArrowDown),
                    //         state: winit::event::ElementState::Pressed,
                    //         ..
                    //     } => {
                    //         let mut renderer = self.ctx.get_mut::<Renderer>();
                    //         renderer.camera.translate(IVec2::new(0, -1) * speed);
                    //         true
                    //     },
                    //     winit::event::KeyEvent {
                    //         logical_key: winit::keyboard::Key::Named(winit::keyboard::NamedKey::ArrowLeft),
                    //         state: winit::event::ElementState::Pressed,
                    //         ..
                    //     } => {
                    //         let mut renderer = self.ctx.get_mut::<Renderer>();
                    //         renderer.camera.translate(IVec2::new(-1, 0) * speed);
                    //         true
                    //     },
                    //     winit::event::KeyEvent {
                    //         logical_key: winit::keyboard::Key::Named(winit::keyboard::NamedKey::ArrowRight),
                    //         state: winit::event::ElementState::Pressed,
                    //         ..
                    //     } => {
                    //         let mut renderer = self.ctx.get_mut::<Renderer>();
                    //         renderer.camera.translate(IVec2::new(1, 0) * speed);
                    //         true
                    //     },
                    //     winit::event::KeyEvent {
                    //         logical_key: winit::keyboard::Key::Named(winit::keyboard::NamedKey::F1),
                    //         state: winit::event::ElementState::Pressed,
                    //         ..
                    //     } => {
                    //         let mut renderer = self.ctx.get_mut::<Renderer>();
                    //         let new_zoom = renderer.camera.zoom() * 2.0;
                    //         renderer.camera.set_zoom(new_zoom);
                    //         true
                    //     },
                    //     winit::event::KeyEvent {
                    //         logical_key: winit::keyboard::Key::Named(winit::keyboard::NamedKey::F2),
                    //         state: winit::event::ElementState::Pressed,
                    //         ..
                    //     } => {
                    //         let mut renderer = self.ctx.get_mut::<Renderer>();
                    //         let new_zoom = renderer.camera.zoom() / 2.0;
                    //         renderer.camera.set_zoom(new_zoom);
                    //         true
                    //     }
                    //     _ => false
                    // }
                },
                WindowEvent::CursorMoved {position, .. } => {
                    let mut input = self.ctx.get_mut::<InputSystem>();
                    input.handle_cursor_movement(position);
                },
                WindowEvent::MouseInput {state, button, ..} => {
                    let mut input = self.ctx.get_mut::<InputSystem>();
                    input.handle_mouse_input(*button, *state);
                }
                _ => {handled = false;}
            }
        } else {
            handled = false;
        }
        handled
    }


    // pub fn use_window_target(&self, target: &winit::event_loop::EventLoopWindowTarget) {
    //     if self.close_requested {
    //         target.exit();
    //     }
    // }
}
