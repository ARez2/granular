use std::pin::Pin;

use geese::*;
use wgpu::Surface;
use winit::window::Window;

use crate::EventLoopSystem;

use super::GPUBackend;


pub struct WindowSystem {
    ctx: GeeseContextHandle<Self>,
    window_handle: Pin<Box<Window>>
}
impl WindowSystem {
    pub fn get(&self) -> &Window {
        &self.window_handle
    }

    pub fn get_mut(&mut self) -> &mut Window {
        &mut self.window_handle
    }

    pub fn create_surface(&self) -> Surface<'static> {
        self.ctx.get::<GPUBackend>().instance().create_surface(&*self.window_handle).expect("Could not create surface for window.")
    }
}
impl GeeseSystem for WindowSystem {
    const DEPENDENCIES: Dependencies = dependencies()
        .with::<Mut<WindowSystem>>()
        .with::<EventLoopSystem>();
    
    fn new(ctx: GeeseContextHandle<Self>) -> Self {
        let event_loop = ctx.get::<EventLoopSystem>();
        let window_handle = winit::window::WindowBuilder::new()
            .with_title("Default Granular Window")
            .with_visible(false)
            .build(event_loop.get()).unwrap();
        
        Self {
            ctx,
            window_handle: Box::pin(window_handle)
        }
    }
}