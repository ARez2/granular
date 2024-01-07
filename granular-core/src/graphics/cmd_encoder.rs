use geese::*;

use super::{SwapchainSystem, GPUBackend};


pub struct CommandEncoderSystem {

}
impl GeeseSystem for CommandEncoderSystem {
    const DEPENDENCIES: Dependencies = dependencies()
        .with::<SwapchainSystem>()
        .with::<GPUBackend>();

    fn new(ctx: GeeseContextHandle<Self>) -> Self {
        Self {

        }
    }
}