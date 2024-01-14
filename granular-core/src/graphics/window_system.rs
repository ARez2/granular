use std::{pin::Pin, sync::Arc};

use geese::*;
use wgpu::Surface;
use winit::window::Window;

use crate::EventLoopSystem;


pub struct WindowSystem {
    window_handle: Arc<Window>
}
impl WindowSystem {
    pub fn window_handle(&self) -> Arc<Window> {
        self.window_handle.clone()
    }
}
impl GeeseSystem for WindowSystem {
    const DEPENDENCIES: Dependencies = dependencies()
        .with::<EventLoopSystem>();
    
    fn new(ctx: GeeseContextHandle<Self>) -> Self {
        let event_loop = ctx.get::<EventLoopSystem>();
        let window_handle = winit::window::WindowBuilder::new()
            .with_title("Default Granular Window")
            .with_visible(false)
            .with_resizable(true)
            .with_window_level(winit::window::WindowLevel::AlwaysOnTop)
            .build(event_loop.get()).unwrap();
        
        Self {
            window_handle: Arc::new(window_handle)
        }
    }
}