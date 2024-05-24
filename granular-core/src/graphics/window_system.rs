use std::sync::Arc;

use geese::*;
use winit::{event_loop::ActiveEventLoop, window::{Window, WindowAttributes}};

use crate::EventLoopSystem;


pub struct WindowSystem {
    windows: Vec<Arc<Window>>
}
impl WindowSystem {
    pub fn window_handle(&self) -> Arc<Window> {
        if self.windows.is_empty() {
            panic!("Tried getting a window handle but no windows exist.");
        }
        self.windows[0].clone()
    }

    pub fn init(&mut self, event_loop: &ActiveEventLoop) {
        let window_attributes = WindowAttributes::default()
            .with_title("Default Granular Window")
            .with_visible(false)
            .with_resizable(true)
            .with_window_level(winit::window::WindowLevel::AlwaysOnTop)
            .with_position(winit::dpi::PhysicalPosition::new(1500, 100));
        let result = event_loop.create_window(window_attributes);
        if let Ok(window) = result {
            self.windows.push(Arc::new(window));
        } else if let Err(e) = result {
            panic!("OS Error while creating a new window: {}", e);
        }
    }
}
impl GeeseSystem for WindowSystem {
    const DEPENDENCIES: Dependencies = dependencies()
        .with::<EventLoopSystem>();
    
    fn new(ctx: GeeseContextHandle<Self>) -> Self {
        Self {
            windows: vec![]
        }
    }
}