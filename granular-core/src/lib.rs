use std::time::{Duration, Instant};

use geese::{GeeseContext, EventQueue};
use glam::Vec2;
use graphics::{Quad, Renderer, WindowSystem};
use log::*;
use rustc_hash::FxHashMap as HashMap;
use winit::{dpi::PhysicalSize, event::WindowEvent};

mod assets;
use assets::AssetSystem;

//mod tick;
mod graphics;

mod eventloop_system;
use eventloop_system::EventLoopSystem;

mod filewatcher;
use filewatcher::FileWatcher;


pub mod events {
    pub struct Initialized {
        
    }

    pub mod timing {
        /// Gets sent out every frame
        pub struct Tick;

        /// Gets sent out every 30 frames
        pub struct Tick30;

        /// Gets sent out every 60 frames
        pub struct Tick60;


        /// Gets sent out every second
        pub struct FixedTick;
        /// Gets sent out every 2.5 seconds
        pub struct FixedTick2500ms;
        /// Gets sent out every 5 seconds
        pub struct FixedTick5000ms;
    }
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
            .with(geese::notify::add_system::<AssetSystem>());

        let now = Instant::now();
        let mut last_ticks = HashMap::default();
        last_ticks.insert(Duration::from_secs_f32(1.0), now);
        last_ticks.insert(Duration::from_secs_f32(2.5), now);
        last_ticks.insert(Duration::from_secs_f32(5.0), now);

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
            let handled = self.handle_winit_events(&event);
            if !handled {
                self.ctx.flush().with(event);
            };
            self.use_window_target(target);
            self.update();
            self.handle_scheduling();
            self.frame += 1;
        }).unwrap();
    }

    pub fn update(&mut self) {
        
    }


    pub fn handle_scheduling(&mut self) {
        let mut buffer = geese::EventBuffer::default()
            .with(events::timing::Tick);
        
        let now = Instant::now();
        self.last_ticks.iter_mut().for_each(|(tickrate, last)| {
            if *last + *tickrate < now {
                *last = now;
                let tickrate_secs = tickrate.as_secs_f32();
                if tickrate_secs == 1.0 {
                    self.ctx.flush().with(events::timing::FixedTick);
                } else if tickrate_secs == 2.5 {
                    self.ctx.flush().with(events::timing::FixedTick2500ms);
                } else if tickrate_secs == 5.0 {
                    self.ctx.flush().with(events::timing::FixedTick5000ms);
                };
            }
        });

        
        
        if self.frame % 60 == 0 {
            buffer = buffer.with(events::timing::Tick60);
        } else if self.frame % 30 == 0 {
            buffer = buffer.with(events::timing::Tick30);
        };
        
        self.ctx.flush().with_buffer(buffer);
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
                    let mut renderer = self.ctx.get_mut::<Renderer>();
                    renderer.resize(new_size);
                    #[cfg(target_os="macos")]
                    graphics.request_redraw();
                    true
                },
                WindowEvent::RedrawRequested => {
                    let mut renderer = self.ctx.get_mut::<Renderer>();
                    renderer.start_frame();
                    //renderer.start_batch();
                    // // TODO: Remove this test drawing
                    // let mut s = 0;
                    // for y in -10..=10 {
                    //     for x in (-10..=10).rev() {
                    //         let col = {
                    //             if s % 3 == 0 {
                    //                 self.tex.clone()
                    //             } else if s % 3 == 1 {
                    //                 self.tex2.clone()
                    //             } else {
                    //                 self.tex3.clone()
                    //             }
                    //         };
                    //         renderer.draw_quad(Vec2::new(x as f32 / 10.0, y as f32 / 10.0), Vec2::new(0.05, 0.05), QuadColoring::Texture(col));
                    //         s += 1;
                    //     };
                    // };

                    renderer.draw_quad(&Quad {
                        center: Vec2::new(-0.5, 0.0),
                        size: Vec2::new(0.2, 0.2),
                        color: wgpu::Color::WHITE,
                        texture: Some(self.tex.clone())
                    });
                    // renderer.draw_quad(&Quad {
                    //     center: Vec2::new(-0.5, 0.5),
                    //     size: Vec2::new(0.2, 0.2),
                    //     color: wgpu::Color::WHITE,
                    //     texture: Some(self.tex.clone())
                    // });
                    renderer.draw_quad(&Quad {
                        center: Vec2::new(0.0, 0.0),
                        size: Vec2::new(0.2, 0.2),
                        color: wgpu::Color::WHITE,
                        texture: Some(self.tex2.clone())
                    });
                    renderer.draw_quad(&Quad {
                        center: Vec2::new(0.5, 0.0),
                        size: Vec2::new(0.2, 0.2),
                        color: wgpu::Color::WHITE,
                        texture: Some(self.tex3.clone())
                    });

                    renderer.end_batch();
                    renderer.flush();
                    renderer.end_frame();
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


    pub fn use_window_target(&self, target: &winit::event_loop::EventLoopWindowTarget) {
        if self.close_requested {
            target.exit();
        }
    }
}
