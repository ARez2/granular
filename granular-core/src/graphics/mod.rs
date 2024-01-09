use geese::*;

mod window_system;
pub use window_system::WindowSystem;

mod gpu;
pub use gpu::GPU;

mod gpu_backend;
pub use gpu_backend::GPUBackend;

mod cmd_encoder;
pub use cmd_encoder::CommandEncoderSystem;

mod swapchain_system;
pub use swapchain_system::SwapchainSystem;

mod fwd_renderer;
use fwd_renderer::ForwardRenderer;

pub struct Graphics {
    ctx: GeeseContextHandle<Self>
}
impl GeeseSystem for Graphics {
    const DEPENDENCIES: Dependencies = dependencies()
        .with::<ForwardRenderer>();

    fn new(ctx: GeeseContextHandle<Self>) -> Self {
        Self {
            ctx
        }
    }
}