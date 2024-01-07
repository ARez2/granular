use geese::*;

use super::{GPUBackend, CommandEncoderSystem};



pub struct ForwardRenderer {
    // Pipelines and shaders
}
impl GeeseSystem for ForwardRenderer {
    const DEPENDENCIES: Dependencies = dependencies()
        .with::<GPUBackend>()
        .with::<CommandEncoderSystem>();

    fn new(ctx: GeeseContextHandle<Self>) -> Self {
        Self {
            
        }
    }
}