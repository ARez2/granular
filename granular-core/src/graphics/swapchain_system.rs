use geese::*;
use wgpu::Surface;

use super::{GPU, GPUBackend, WindowSystem};


pub struct SwapchainSystem {
    surface: Surface<'static>
}
impl GeeseSystem for SwapchainSystem {
    const DEPENDENCIES: Dependencies = dependencies()
        .with::<GPU>()
        .with::<GPUBackend>()
        .with::<WindowSystem>();

    fn new(ctx: GeeseContextHandle<Self>) -> Self {
        let instance = ctx.get::<GPUBackend>();
        let instance = instance.instance();
        let window = ctx.get::<WindowSystem>();
        let window = window.get();
        Self {
            surface: instance.create_surface(window).expect("Cannot create surface")
        }
    }
}