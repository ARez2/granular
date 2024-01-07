use geese::*;
use winit::window::Window;

use crate::EventLoopSystem;


pub struct WindowSystem {
    window: Window
}
impl WindowSystem {
    pub fn get(&self) -> &Window {
        &self.window
    }

    pub fn get_mut(&mut self) -> &mut Window {
        &mut self.window
    }
}
impl GeeseSystem for WindowSystem {
    const DEPENDENCIES: Dependencies = dependencies()
        .with::<EventLoopSystem>();
    
    fn new(ctx: GeeseContextHandle<Self>) -> Self {
        let event_loop = ctx.get::<EventLoopSystem>();
        let window = winit::window::WindowBuilder::new()
            .with_title("Default Granular Window")
            .with_visible(false)
            .build(event_loop.get()).unwrap();
        
        Self {
            window
        }
    }
}