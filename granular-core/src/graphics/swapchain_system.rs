use geese::*;

use super::{GPUBackend, WindowSystem};


pub struct SwapchainSystem {

}
impl GeeseSystem for SwapchainSystem {
    const DEPENDENCIES: Dependencies = dependencies()
        .with::<GPUBackend>()
        .with::<WindowSystem>();

    fn new(ctx: GeeseContextHandle<Self>) -> Self {
        Self {

        }
    }
}